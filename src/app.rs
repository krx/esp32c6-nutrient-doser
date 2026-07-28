use std::{sync::Arc, time::Duration};

use embedded_svc::http::{
    client::{Client, Response},
    Headers, Method,
};
use http::header::ACCEPT;

use axum::{
    body::Body,
    extract::{FromRef, State},
    http::{Request, StatusCode, Uri},
    middleware::Next,
    routing::{get, post},
    Json, Router,
};
use esp_idf_svc::{
    hal::reset::restart,
    http::client::{Configuration, EspHttpConnection},
    nvs::{EspCustomNvs, EspCustomNvsPartition},
    ota::{EspOta, FirmwareInfo},
    sys::{EspError, ESP_ERR_IMAGE_INVALID, ESP_ERR_INVALID_RESPONSE, ESP_FAIL},
};

use log::{error, info};
use mime::APPLICATION_OCTET_STREAM;
use serde::{Deserialize, Serialize};
use tokio::{
    net::TcpListener,
    sync::{mpsc, oneshot, Mutex, RwLock},
    time::interval,
};
use tower_http::cors::{self, CorsLayer};

use crate::{
    online_linreg::OnlineLinearRegression,
    rmt_drv8825::{DRV8825, MAX_RPM},
};

#[macro_export]
macro_rules! esp_err {
    ($x:ident) => {
        Err(EspError::from_infallible::<$x>())
    };
}

const FIRMWARE_DOWNLOAD_CHUNK_SIZE: usize = 1024 * 20;
const FIRMWARE_MAX_SIZE: usize = 0x3f0000; // Max size of each app partition
const FIRMWARE_MIN_SIZE: usize = size_of::<FirmwareInfo>() + 1024;

const BIND_IP: &str = "0.0.0.0";
const PORT: u16 = 80;

pub const NVS_NS: &str = "storage";
const NVS_TAG_MOTORS: &str = "motors";

#[derive(Serialize, Deserialize)]
struct StepperMotor {
    id: u32, // using the EN pin # to identify motors

    #[serde(skip)]
    driver: Option<DRV8825>,

    // Estimates slope (steps/mL) and intercept (# steps before any liquid is dispensed)
    model: OnlineLinearRegression,
}

impl Default for StepperMotor {
    fn default() -> Self {
        Self {
            id: Default::default(),
            driver: None,
            model: OnlineLinearRegression::new(),
        }
    }
}

impl StepperMotor {
    async fn unprime(&mut self) {
        info!("Unpriming motor {}", self.id);
        let prime_steps = self.model.intercept();
        if let Some(drv) = &mut self.driver {
            drv.step_by(-2.0 * prime_steps).await.unwrap();
            drv.reset_position();
        }
    }

    async fn unprime_slow(&mut self) {
        info!("Unpriming motor {}", self.id);
        let prime_steps = self.model.intercept();
        if let Some(drv) = &mut self.driver {
            drv.step_by(-2.0 * prime_steps).await.unwrap();
            drv.step_by_speed(-4.0 * prime_steps, 0.2 * MAX_RPM).await.unwrap();
            drv.reset_position();
        }
    }

    async fn keep_primed(&mut self) {
        info!("Priming/Unpriming motor {}", self.id);
        let prime_steps = self.model.intercept();
        if let Some(drv) = &mut self.driver {
            // This should at least get liquid past the pump itself
            if drv.get_position() == 0 {
                drv.step_by(0.75 * prime_steps).await.unwrap();
            }

            // Then 2 unprime passes to make sure liquid isn't stuck in the middle of the hose
            drv.step_by(-2.0 * prime_steps).await.unwrap();
            drv.step_by_speed(-2.0 * prime_steps, 0.2 * MAX_RPM).await.unwrap();
            drv.reset_position();
        }
    }

    async fn dispense_ml(&mut self, ml: f64) {
        if let Some(drv) = &mut self.driver {
            // First, unprime slightly to get rid of any liquid at the start of the hose
            // that way every time we dispense starts from a completely empty hose on both sides
            drv.step_by(-0.25 * self.model.intercept()).await.unwrap();

            // Then, dispense the requested amount according to the model
            drv.step_by(self.model.predict(ml)).await.unwrap();

            // Unprime again completely to return to the starting point
            self.unprime().await;
        }
    }
}

#[derive(Serialize, Clone, Copy)]
enum AppStatus {
    IDLE,
    RUNNING,
    OTA,
    PRIMING,
}

#[derive(Clone, FromRef)]
struct AppState {
    motors: Arc<Mutex<Vec<StepperMotor>>>,
    nvs: Arc<RwLock<EspCustomNvs>>,
    status: Arc<RwLock<AppStatus>>,
    timer_reset_tx: mpsc::Sender<()>,
}

impl AppState {
    async fn create_config(&self, drivers: Vec<DRV8825>) {
        info!("Creating new motor config");
        for drv in drivers {
            self.add_config_entry(drv).await;
        }
    }

    async fn add_config_entry(&self, drv: DRV8825) {
        self.motors.lock().await.push(StepperMotor {
            id: drv.id(),
            driver: Some(drv),
            model: OnlineLinearRegression::new(),
        });
    }

    async fn save_state(&self) {
        match serde_json::to_string(&*self.motors.lock().await) {
            Ok(state_str) => {
                info!("Writing state to nvs: {state_str}");
                if let Err(e) = self
                    .nvs
                    .write()
                    .await
                    .set_str(NVS_TAG_MOTORS, state_str.as_str())
                {
                    error!("Failed to write state to nvs: {e}");
                }
            }
            Err(e) => error!("Failed to serialize state: {e}"),
        };
    }

    async fn set_status(&self, status: AppStatus) {
        *self.status.write().await = status;
    }

    async fn reset_timer(&self) {
        self.timer_reset_tx.send(()).await.unwrap();
    }
}

pub async fn run(drivers: Vec<DRV8825>) -> anyhow::Result<()> {
    info!("Starting app...");

    let (timer_reset_tx, mut timer_reset_rx) = mpsc::channel::<()>(1);

    let state = AppState {
        motors: Arc::new(Mutex::new(Vec::new())),
        nvs: Arc::new(RwLock::new(EspCustomNvs::new(
            EspCustomNvsPartition::take("nvs")?,
            NVS_NS,
            true,
        )?)),
        status: Arc::new(RwLock::new(AppStatus::IDLE)),
        timer_reset_tx,
    };

    // Load motor config if it exists, or create it
    match state.nvs.read().await.str_len(NVS_TAG_MOTORS) {
        Ok(Some(len)) => {
            info!("Loading existing motor config");
            let mut buf = vec![0_u8; len];
            match state
                .nvs
                .read()
                .await
                .get_str(NVS_TAG_MOTORS, buf.as_mut_slice())
            {
                Ok(Some(config)) => {
                    info!("Read config from nvs: {config}");
                    match serde_json::from_str::<Vec<StepperMotor>>(config) {
                        Ok(loaded_motors) => {
                            *state.motors.lock().await = loaded_motors;
                            for drv in drivers {
                                if let Some(m) = state
                                    .motors
                                    .lock()
                                    .await
                                    .iter_mut()
                                    .find(|m| m.id == drv.id())
                                {
                                    info!("Matched motor {}", m.id);
                                    m.driver = Some(drv);
                                    continue;
                                }
                                info!("Adding config entry for new motor: {}", drv.id());
                                state.add_config_entry(drv).await;
                            }
                            state.motors.lock().await.retain(|m| m.driver.is_some());
                        }
                        _ => state.create_config(drivers).await,
                    };
                }
                _ => state.create_config(drivers).await,
            };
        }
        _ => state.create_config(drivers).await,
    };
    state.save_state().await;

    // Start timer to periodically prime/unprime all motors
    let _state = state.clone();
    tokio::spawn(async move {
        let mut timer = interval(Duration::from_mins(60));
        timer.reset();
        loop {
            tokio::select! {
                _ = timer_reset_rx.recv() => {
                    info!("Resetting unprime timer");
                    timer.reset();
                }
                _ = timer.tick() => {
                    info!("Auto-unpriming all motors due to inactivity");
                    _state.set_status(AppStatus::PRIMING).await;
                    for m in _state.motors.lock().await.iter_mut() {
                        m.keep_primed().await;
                    }
                    timer.reset();
                    _state.set_status(AppStatus::IDLE).await;
                }
            };
        }
    });

    info!("Config loaded, starting app...");
    let app = Router::new()
        .route("/", get(root))
        .route("/full-status", get(get_full_status))
        .route("/status", get(get_status))
        .route("/debug/step", post(debug_step))
        .route("/debug/clear-config", post(debug_clear_config))
        .route("/dispense", post(dispense))
        .route("/unprime", post(unprime))
        .route("/unprime-all", post(unprime_all))
        .route("/calibrate", post(calibrate))
        .route("/clear-calibration", post(clear_calibration))
        .route("/dose", post(dose_solution))
        .route("/reboot", get(reboot))
        .route("/ota", post(handle_ota))
        .with_state(state.clone())
        .layer(
            CorsLayer::new()
                .allow_origin(cors::Any)
                .allow_methods(cors::Any)
                .allow_headers(cors::Any),
        )
        .route_layer(axum::middleware::from_fn(
            // this spawns every route request to protect against cancellation
            |req: Request<Body>, next: Next| async move {
                tokio::task::spawn(next.run(req)).await.unwrap()
            },
        ));

    info!("Binding to {BIND_IP}:{PORT}...");
    let listener = TcpListener::bind(format!("{BIND_IP}:{PORT}")).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn root() -> Json<String> {
    Json(format!("Nutrient doser {}", env!("CARGO_PKG_VERSION")))
}

#[derive(Serialize)]
struct MotorStatus {
    idx: usize,
    id: u32,
    position: i32,
    model: OnlineLinearRegression,
}

#[derive(Serialize)]
struct FullStatus {
    version: &'static str,
    status: AppStatus,
    num_motors: usize,
    motors: Vec<MotorStatus>,
}

async fn get_full_status(State(state): State<AppState>) -> Json<FullStatus> {
    let _motors = state.motors.lock().await;
    Json(FullStatus {
        num_motors: _motors.len(),
        motors: _motors
            .iter()
            .enumerate()
            .map(|(idx, m)| MotorStatus {
                idx,
                id: m.id,
                position: m.driver.as_ref().unwrap().get_position(),
                model: m.model,
            })
            .collect(),
        version: env!("CARGO_PKG_VERSION"),
        status: *state.status.read().await,
    })
}

#[derive(Serialize)]
struct Status {
    status: AppStatus,
}

async fn get_status(State(state): State<AppState>) -> Json<Status> {
    Json(Status {
        status: *state.status.read().await,
    })
}

#[derive(Deserialize)]
struct DispenseSingle {
    motor_idx: usize,
    ml: f64,
}

#[derive(Deserialize)]
struct DispenseReq {
    reqs: Vec<DispenseSingle>,
}

async fn dispense(
    State(state): State<AppState>,
    Json(req): Json<DispenseReq>,
) -> StatusCode {
    let mut res = StatusCode::OK;
    let mut motors = state.motors.lock().await;
    for r in req.reqs.iter() {
        match motors.get_mut(r.motor_idx) {
            Some(motor) => {
                info!("Dispensing {}ml of liquid #{}", r.ml, r.motor_idx);
                state.reset_timer().await;
                state.set_status(AppStatus::RUNNING).await;
                motor.dispense_ml(r.ml).await;
                state.set_status(AppStatus::IDLE).await;
            }
            None => res = StatusCode::BAD_REQUEST,
        }
    }
    res
}

#[derive(Deserialize)]
struct DebugStepReq {
    motor_idx: usize,
    steps: f64,
    speed: Option<f64>,
}

async fn debug_step(
    State(state): State<AppState>,
    Json(req): Json<DebugStepReq>,
) -> StatusCode {
    let res = match state.motors.lock().await.get_mut(req.motor_idx) {
        Some(motor) => {
            if let Some(drv) = &mut motor.driver {
                state.reset_timer().await;
                state.set_status(AppStatus::RUNNING).await;
                drv.step_by_speed(req.steps, req.speed.unwrap_or(MAX_RPM))
                    .await
                    .unwrap();
                state.set_status(AppStatus::IDLE).await;
            }
            StatusCode::OK
        }
        None => StatusCode::BAD_REQUEST,
    };

    res
}

async fn debug_clear_config(State(state): State<AppState>) {
    state.motors.lock().await.clear();
    state.save_state().await;
    restart();
}

#[derive(Deserialize)]
struct UnprimeReq {
    motor_idx: usize,
}

async fn unprime(
    State(state): State<AppState>,
    Json(req): Json<UnprimeReq>,
) -> StatusCode {
    match state.motors.lock().await.get_mut(req.motor_idx) {
        Some(motor) => {
            state.reset_timer().await;
            state.set_status(AppStatus::RUNNING).await;
            motor.unprime().await;
            state.set_status(AppStatus::IDLE).await;
            StatusCode::OK
        }
        None => StatusCode::BAD_REQUEST,
    }
}

async fn unprime_all(State(state): State<AppState>) -> StatusCode {
    state.reset_timer().await;
    state.set_status(AppStatus::RUNNING).await;
    for m in state.motors.lock().await.iter_mut() {
        m.unprime_slow().await;
    }
    state.set_status(AppStatus::IDLE).await;
    StatusCode::OK
}

#[derive(Deserialize)]
struct CalibrateReq {
    motor_idx: usize,
    expected: f64,
    actual: f64,
}

async fn calibrate(
    State(state): State<AppState>,
    Json(req): Json<CalibrateReq>,
) -> StatusCode {
    let res = match state.motors.lock().await.get_mut(req.motor_idx) {
        Some(motor) => {
            let steps = motor.model.predict(req.expected);
            motor.model.update(req.actual, steps);
            StatusCode::OK
        }
        None => StatusCode::BAD_REQUEST,
    };

    state.save_state().await;
    res
}

#[derive(Deserialize)]
struct ClearCalibrationReq {
    motor_idx: usize,
}

async fn clear_calibration(
    State(state): State<AppState>,
    Json(req): Json<ClearCalibrationReq>,
) -> StatusCode {
    let res = match state.motors.lock().await.get_mut(req.motor_idx) {
        Some(motor) => {
            motor.model = OnlineLinearRegression::new();
            StatusCode::OK
        }
        None => StatusCode::BAD_REQUEST,
    };

    state.save_state().await;
    res
}

#[derive(Deserialize)]
enum VolUnit {
    #[serde(alias = "ml", alias = "mL")]
    Ml,

    #[serde(alias = "l")]
    L,

    #[serde(alias = "gal")]
    Gal,

    #[serde(alias = "fl oz", alias = "Fl Oz", alias = "floz")]
    FlOz,
}

impl VolUnit {
    fn scale_to_ml(&self) -> f64 {
        match self {
            VolUnit::Ml => 1.0,
            VolUnit::L => 1000.0,
            VolUnit::Gal => 3785.41,
            VolUnit::FlOz => 29.5735,
        }
    }
}

#[derive(Deserialize)]
struct NutrientInfo {
    name: String,
    motor_idx: usize,
    ml_per_gal: f64,
}

#[derive(Deserialize)]
struct DoseSolutionReq {
    nutrients: Vec<NutrientInfo>,
    target_amount: f64,
    target_unit: VolUnit,
}

async fn dose_solution(
    State(state): State<AppState>,
    Json(req): Json<DoseSolutionReq>,
) -> StatusCode {
    let solution_ml = req.target_amount * req.target_unit.scale_to_ml();
    let solution_gal = solution_ml / VolUnit::Gal.scale_to_ml();
    info!("Dosing solution for {solution_gal} gallons of water ({solution_ml} mL)");
    state.reset_timer().await;
    state.set_status(AppStatus::RUNNING).await;
    let mut motors = state.motors.lock().await;
    for nutrient in req.nutrients {
        if let Some(motor) = motors.get_mut(nutrient.motor_idx) {
            let ml_needed = solution_gal * nutrient.ml_per_gal;
            if ml_needed > 0.0 {
                info!("Dispensing {ml_needed}mL of {}", nutrient.name);
                motor.dispense_ml(ml_needed).await;
            }
        }
    }
    state.set_status(AppStatus::IDLE).await;
    StatusCode::OK
}

async fn reboot() {
    restart();
}

#[derive(Deserialize)]
struct OtaReq {
    #[serde(with = "http_serde_ext::uri")]
    uri: Uri,
}

async fn handle_ota(State(state): State<AppState>, Json(req): Json<OtaReq>) -> StatusCode {
    state.set_status(AppStatus::OTA).await;
    match do_ota(req.uri).await {
        Ok(_) => {
            info!("OTA download successful! rebooting to new image...");
            restart();
        }
        Err(e) => {
            error!("OTA failed! - {e} {e:?}");

            state.set_status(AppStatus::IDLE).await;
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };
}

fn handle_ota_resp(mut resp: Response<&mut EspHttpConnection>) -> Result<(), EspError> {
    if resp.status() != 200 {
        error!("Unexpected HTTP response: {}", resp.status());
        return esp_err!(ESP_ERR_INVALID_RESPONSE);
    }

    // Check firmware size
    let file_size = resp.content_len().unwrap_or(0) as usize;
    if file_size <= FIRMWARE_MIN_SIZE {
        error!("Firmware size ({file_size}) is too small!");
        return esp_err!(ESP_ERR_IMAGE_INVALID);
    }
    if file_size > FIRMWARE_MAX_SIZE {
        error!("Firmware size ({file_size}) is too large!");
        return esp_err!(ESP_ERR_IMAGE_INVALID);
    }

    // Start OTA
    let mut ota = EspOta::new()?;

    // For some reason the call to EspOta::get_firmware_info inside these raises ESP_ERR_INVALID_SIZE now???
    // just logging these, so commenting out for now
    // info!(
    //     "CURRENT SLOTS (BOOT, RUN, UPD): ({}, {}, {})",
    //     ota.get_boot_slot()?.label,
    //     ota.get_running_slot()?.label,
    //     ota.get_update_slot()?.label
    // );

    let mut upd = ota.initiate_update()?;
    let mut buf = vec![0; FIRMWARE_DOWNLOAD_CHUNK_SIZE];
    let mut total: usize = 0;
    let ota_res = loop {
        let n = resp.read(&mut buf).unwrap_or_default();
        total += n;

        if n > 0 {
            if let Err(e) = upd.write(&buf[..n]) {
                error!("Failed to write OTA chunk: {e:?}");
                break Err(e);
            }
            info!(
                "OTA progress: {:.2}%",
                100.0 * total as f32 / file_size as f32
            );
        }

        if total >= file_size {
            break Ok(());
        }
    };

    // TODO: checksum

    if ota_res.is_err() || total < file_size {
        error!("Error while writing OTA, aborting");
        error!("Total of {total} out of {file_size} bytes received");
        return upd.abort();
    }

    // OTA was successful if we reach this
    upd.complete()
}

async fn do_ota(uri: Uri) -> Result<(), EspError> {
    let (signal_tx, signal_rx) = oneshot::channel();
    let req_task = tokio::task::spawn_blocking(move || -> Result<(), EspError> {
        let mut client = Client::wrap(EspHttpConnection::new(&Configuration {
            buffer_size: Some(4096),
            ..Default::default()
        })?);

        let uri_str = uri.to_string();
        let headers = [(ACCEPT.as_str(), APPLICATION_OCTET_STREAM.as_ref())];
        let res = match client.request(Method::Get, &uri_str, &headers) {
            Ok(req) => match req.submit() {
                Ok(resp) => handle_ota_resp(resp),
                Err(e) => {
                    error!("Failed to send request! {e:?}");
                    esp_err!(ESP_FAIL)
                }
            },
            Err(e) => {
                error!("Failed to build request! {e:?}");
                esp_err!(ESP_FAIL)
            }
        };

        // Signal the outer await to exit before this thread is done
        signal_tx.send(res).unwrap();
        res
    });

    // await a signal instead of waiting on the thread so other tasks can keep running
    let ota_success = signal_rx.await.unwrap();
    if let Err(e) = req_task.await {
        error!("Blocking OTA task didn't join properly ???: {e:?}");
    }

    ota_success
}

use std::sync::Arc;

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
    sync::{oneshot, Mutex, RwLock},
};
use tower_http::cors::{self, CorsLayer};

use crate::rmt_drv8825::DRV8825;

#[macro_export]
macro_rules! esp_err {
    ($x:ident) => {
        Err(EspError::from_infallible::<$x>())
    };
}

const FIRMWARE_DOWNLOAD_CHUNK_SIZE: usize = 1024 * 20;
const FIRMWARE_MAX_SIZE: usize  = 0x3f0000; // Max size of each app partition
const FIRMWARE_MIN_SIZE: usize  = size_of::<FirmwareInfo>() + 1024;

const BIND_IP: &str = "0.0.0.0";
const PORT: u16 = 80;

pub const NVS_NS: &str = "storage";
const NVS_TAG_MOTORS: &str = "motors";

#[derive(Serialize, Deserialize)]
struct StepperMotor {
    id: u32, // using the EN pin # to identify motors
    #[serde(skip)] driver: Option<DRV8825>,
    ml_per_step: f64, // Estimation of the amount of liquid dispensed per full step
    prime_steps: u32, // Number of steps needed to pull liquid through all the tubing up to the nozzle
}

impl Default for StepperMotor {
    fn default() -> Self {
        Self {
            id: Default::default(),
            driver: None,
            ml_per_step: 0.0032,  // From testing, this should be pretty close to start with
            prime_steps: 0,
        }
    }
}

impl StepperMotor {
    fn is_primed(&self) -> bool {
        match &self.driver {
            Some(drv) => drv.get_position() > 0,
            None => false,
        }
    }

    async fn ensure_primed(&mut self) {
        if !self.is_primed() {
            if let Some(drv) = &mut self.driver {
                info!("Priming motor {}", self.id);
                drv.step_by(self.prime_steps as f64).await.unwrap()
            }
        }
    }

    async fn unprime(&mut self) {
        info!("Unpriming motor {}", self.id);
        if let Some(drv) = &mut self.driver {
            drv.step_by(-2.0 * self.prime_steps as f64).await.unwrap();
            drv.reset_position();
        }
    }

    async fn dispense_ml(&mut self, ml: f64) {
        self.ensure_primed().await;
        if let Some(drv) = &mut self.driver {
            drv.step_by((ml / self.ml_per_step).floor() as f64)
                .await
                .unwrap();

            // Back off slightly to prevent extra liquid dripping out from pressure
            drv.step_by(-50.0).await.unwrap();
        }
    }
}

#[derive(Serialize, Clone, Copy)]
enum AppStatus {
    IDLE,
    RUNNING,
    OTA
}

// type SharedState = Arc<Mutex<AppState>>;
// type SharedStatus = Arc<RwLock<AppStatus>>;
#[derive(Clone, FromRef)]
struct AppState {
    motors: Arc<Mutex<Vec<StepperMotor>>>,
    nvs: Arc<RwLock<EspCustomNvs>>,
    status: Arc<RwLock<AppStatus>>
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
            ..Default::default()
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
}

pub async fn run(drivers: Vec<DRV8825>) -> anyhow::Result<()> {
    info!("Starting app...");

    let state = AppState {
        motors: Arc::new(Mutex::new(Vec::new())),
        nvs: Arc::new(RwLock::new(EspCustomNvs::new(
            EspCustomNvsPartition::take("nvs")?,
            NVS_NS,
            true,
        )?)),
        status: Arc::new(RwLock::new(AppStatus::IDLE)),
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

    info!("Config loaded, starting app...");
    let app = Router::new()
        .route("/", get(root))
        .route("/full-status", get(get_full_status))
        .route("/status", get(get_status))
        .route("/debug/step", post(debug_step))
        .route("/debug/calibrate", post(debug_calibrate))
        .route("/debug/clear-config", post(debug_clear_config))
        .route("/dispense", post(dispense))
        .route("/update-prime", post(update_prime))
        .route("/unprime", post(unprime))
        .route("/unprime-all", post(unprime_all))
        .route("/calibrate", post(calibrate))
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
    is_primed: bool,
    prime_steps: u32,
    ml_per_step: f64,
}

#[derive(Serialize)]
struct FullStatus {
    num_motors: usize,
    motors: Vec<MotorStatus>,
    version: &'static str,
    status: AppStatus,
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
                is_primed: m.is_primed(),
                prime_steps: m.prime_steps,
                ml_per_step: m.ml_per_step,
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
    Json(req): Json<DispenseReq>
) -> StatusCode {
    let mut res = StatusCode::OK;
    for r in req.reqs.iter() {
        match state.motors.lock().await.get_mut(r.motor_idx) {
            Some(motor) => {
                info!("Dispensing {}ml of liquid #{}", r.ml, r.motor_idx);
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
}

async fn debug_step(
    State(state): State<AppState>,
    Json(req): Json<DebugStepReq>
) -> StatusCode {
    let res = match state.motors.lock().await.get_mut(req.motor_idx) {
        Some(motor) => {
            if let Some(drv) = &mut motor.driver {
                state.set_status(AppStatus::RUNNING).await;
                drv.step_by(req.steps).await.unwrap();
                state.set_status(AppStatus::IDLE).await;
            }
            StatusCode::OK
        }
        None => StatusCode::BAD_REQUEST,
    };

    res
}

#[derive(Deserialize)]
struct DebugCalibrateReq {
    motor_idx: usize,
    value: f64,
}

async fn debug_calibrate(
    State(state): State<AppState>,
    Json(req): Json<DebugCalibrateReq>
) -> StatusCode {
    let res = match state.motors.lock().await.get_mut(req.motor_idx) {
        Some(motor) => {
            motor.ml_per_step = req.value;
            StatusCode::OK
        }
        None => StatusCode::BAD_REQUEST,
    };

    state.save_state().await;
    res
}


async fn debug_clear_config(State(state): State<AppState>) {
    state.motors.lock().await.clear();
    state.save_state().await;
    restart();
}

#[derive(Deserialize)]
struct UpdatePrimeReq {
    motor_idx: usize,
    prime_steps: u32,
}

async fn update_prime(
    State(state): State<AppState>,
    Json(req): Json<UpdatePrimeReq>,
) -> StatusCode {
    let res = match state.motors.lock().await.get_mut(req.motor_idx) {
        Some(motor) => {
            state.set_status(AppStatus::RUNNING).await;
            motor.unprime().await;
            motor.prime_steps = req.prime_steps;
            motor.ensure_primed().await;
            state.set_status(AppStatus::IDLE).await;
            StatusCode::OK
        }
        None => StatusCode::BAD_REQUEST,
    };
    state.save_state().await;
    res
}

#[derive(Deserialize)]
struct UnprimeReq {
    motor_idx: usize,
}

async fn unprime(
    State(state): State<AppState>,
    Json(req): Json<UnprimeReq>
) -> StatusCode {
    match state.motors.lock().await.get_mut(req.motor_idx) {
        Some(motor) => {
            state.set_status(AppStatus::RUNNING).await;
            motor.unprime().await;
            state.set_status(AppStatus::IDLE).await;
            StatusCode::OK
        }
        None => StatusCode::BAD_REQUEST,
    }
}

async fn unprime_all(State(state): State<AppState>) -> StatusCode {
    state.set_status(AppStatus::RUNNING).await;
    for m in state.motors.lock().await.iter_mut() {
        m.unprime().await;
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
    Json(req): Json<CalibrateReq>
) -> StatusCode {
    let res = match state.motors.lock().await.get_mut(req.motor_idx) {
        Some(motor) => {
            let orig_steps = req.expected / motor.ml_per_step;
            motor.ml_per_step = req.actual / orig_steps;
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

    #[serde(alias = "fl oz", alias="Fl Oz", alias="floz")]
    FlOz
}

impl VolUnit {
    fn scale_to_ml(&self) -> f64 {
        match self {
            VolUnit::Ml => 1.0,
            VolUnit::L => 1000.0,
            VolUnit::Gal => 3785.41,
            VolUnit::FlOz => 29.5735
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
    Json(req): Json<DoseSolutionReq>
) -> StatusCode {
    let solution_ml = req.target_amount * req.target_unit.scale_to_ml();
    let solution_gal = solution_ml / VolUnit::Gal.scale_to_ml();
    info!("Dosing solution for {solution_gal} gallons of water ({solution_ml} mL)");
    state.set_status(AppStatus::RUNNING).await;
    for nutrient in req.nutrients {
        if let Some(motor) = state.motors.lock().await.get_mut(nutrient.motor_idx) {
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

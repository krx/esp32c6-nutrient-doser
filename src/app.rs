use std::time::Duration;
use std::sync::Arc;

use embedded_svc::http::{
    client::{Client, Response},
    Headers, Method,
};
use http::header::ACCEPT;

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode, Uri},
    middleware::Next,
    routing::{get, post},
    Json, Router,
};
use esp_idf_svc::{
    hal::{
        gpio::{AnyIOPin, Output, PinDriver},
        reset::restart,
    },
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
    sync::{oneshot, Mutex},
    time::sleep,
};
use tower_http::cors::{self, CorsLayer};

#[macro_export]
macro_rules! esp_err {
    ($x:ident) => {
        Err(EspError::from_infallible::<$x>())
    };
}

const FIRMWARE_DOWNLOAD_CHUNK_SIZE: usize = 1024 * 20;
const FIRMWARE_MAX_SIZE: usize  = 0x1f0000; // Max size of each app partition
const FIRMWARE_MIN_SIZE: usize  = size_of::<FirmwareInfo>() + 1024;

const BIND_IP: &str = "0.0.0.0";
const PORT: u16 = 80;

pub const NVS_NS: &str = "storage";
const NVS_TAG_MOTORS: &str = "motors";

type MotorPin = PinDriver<'static, AnyIOPin, Output>;

#[derive(Serialize, Deserialize)]
struct Motor {
    id: i32,
    #[serde(skip)] pin: Option<MotorPin>,
    ml_per_sec: f64,
}

impl Default for Motor {
    fn default() -> Self {
        Self {
            id: Default::default(),
            pin: None,
            ml_per_sec: 1.0,
        }
    }
}

impl Motor {
    async fn run_for(&mut self, time: Duration) {
        if let Some(pin) = &mut self.pin {
            pin.set_high().unwrap();
            sleep(time).await;
            pin.set_low().unwrap();
        }
    }

    async fn dispense_ml(&mut self, ml: f64) {
        self.run_for(Duration::from_secs_f64(ml / self.ml_per_sec))
            .await
    }
}

type SharedState = Arc<Mutex<AppState>>;
struct AppState {
    motors: Vec<Motor>,
    nvs: EspCustomNvs,
}

impl AppState {
    fn create_config(&mut self, motors: Vec<MotorPin>) {
        info!("Creating new motor config");
        for m in motors {
            self.motors.push(Motor {
                id: m.pin(),
                pin: Some(m),
                ..Default::default()
            });
        }
        self.save_state();
    }

    fn save_state(&mut self) {
        match serde_json::to_string(&self.motors) {
            Ok(state_str) => {
                info!("Writing state to nvs: {state_str}");
                if let Err(e) = self.nvs.set_str(NVS_TAG_MOTORS, state_str.as_str()) {
                    error!("Failed to write state to nvs: {e}");
                }
            }
            Err(e) => error!("Failed to serialize state: {e}"),
        };
    }
}

pub async fn run(motors: Vec<MotorPin>) -> anyhow::Result<()> {
    info!("Starting app...");

    let mut _state = AppState {
        motors: Vec::new(),
        nvs: EspCustomNvs::new(EspCustomNvsPartition::take("nvs")?, NVS_NS, true)?,
    };

    // Load motor config if it exists, or create it
    match _state.nvs.str_len(NVS_TAG_MOTORS) {
        Ok(Some(len)) => {
            info!("Loading existing motor config");
            let mut buf = vec![0_u8; len];
            match _state.nvs.get_str(NVS_TAG_MOTORS, buf.as_mut_slice()) {
                Ok(Some(config)) => {
                    info!("Read config from nvs: {config}");
                    match serde_json::from_str::<Vec<Motor>>(config) {
                        Ok(loaded_motors) => {
                            _state.motors = loaded_motors;
                            for mpin in motors {
                                if let Some(m) = _state.motors.iter_mut().find(|m| m.id == mpin.pin()) {
                                    info!("Matched motor {}", m.id);
                                    m.pin = Some(mpin);
                                }
                            }
                        }
                        _ => _state.create_config(motors),
                    };
                }
                _ => _state.create_config(motors),
            };
        }
        _ => _state.create_config(motors),
    };

    let state = Arc::new(Mutex::new(_state));
    let app = Router::new()
        .route("/", get(root))
        .route("/info", get(get_info))
        .route("/dispense", post(dispense))
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

    let listener = TcpListener::bind(format!("{BIND_IP}:{PORT}")).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn root() -> Json<String> {
    Json(format!("Nutrient doser {}", env!("CARGO_PKG_VERSION")))
}

#[derive(Serialize)]
struct Info {
    num_motors: usize,
}

async fn get_info(State(state): State<SharedState>) -> Json<Info> {
    Json(Info {
        num_motors: state.lock().await.motors.len(),
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

async fn dispense(State(state): State<SharedState>, Json(req): Json<DispenseReq>) -> StatusCode {
    let mut resp = StatusCode::OK;
    for r in req.reqs.iter() {
        match state.lock().await.motors.get_mut(r.motor_idx) {
            Some(motor) => {
                let dispense_time = r.ml / motor.ml_per_sec;
                info!("Dispensing liquid #{} for {dispense_time}s", r.motor_idx);
                motor.run_for(Duration::from_secs_f64(dispense_time)).await;
            }
            None => resp = StatusCode::BAD_REQUEST,
        }
    }
    resp
}

#[derive(Deserialize)]
struct CalibrateReq {
    motor_idx: usize,
    expected: f64,
    actual: f64,
}

async fn calibrate(State(state): State<SharedState>, Json(req): Json<CalibrateReq>) -> StatusCode {
    let mut _state = state.lock().await;
    match _state.motors.get_mut(req.motor_idx) {
        Some(motor) => {
            let orig_dispense_time = req.expected / motor.ml_per_sec;
            motor.ml_per_sec = req.actual / orig_dispense_time;
            _state.save_state();

            StatusCode::OK
        }
        None => StatusCode::BAD_REQUEST,
    }
}

#[derive(Deserialize)]
enum VolUnit {
    Ml,
    L,
    Gal,
}

impl VolUnit {
    fn scale_to_ml(&self) -> f64 {
        match self {
            VolUnit::Ml => 1.0,
            VolUnit::L => 1000.0,
            VolUnit::Gal => 3785.41,
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

async fn dose_solution(State(state): State<SharedState>, Json(req): Json<DoseSolutionReq>) {
    let solution_ml = req.target_amount * req.target_unit.scale_to_ml();
    let solution_gal = solution_ml / VolUnit::Gal.scale_to_ml();
    info!("Dosing solution for {solution_gal} gallons of water ({solution_ml} mL)");
    for nutrient in req.nutrients {
        if let Some(motor) = state.lock().await.motors.get_mut(nutrient.motor_idx) {
            let ml_needed = solution_gal * nutrient.ml_per_gal;

            info!("Dispensing {ml_needed}mL of {}", nutrient.name);
            motor.dispense_ml(ml_needed).await;
        }
    }
}

async fn reboot() {
    restart();
}

#[derive(Deserialize)]
struct OtaReq {
    #[serde(with = "http_serde_ext::uri")]
    uri: Uri,
}

async fn handle_ota(Json(req): Json<OtaReq>) {
    match do_ota(req.uri).await {
        Ok(_) => {
            info!("OTA download successful! rebooting to new image...");
            restart();
        }
        Err(e) => error!("OTA failed! - {e}"),
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
    info!(
        "CURRENT SLOTS (BOOT, RUN, UPD): ({}, {}, {})",
        ota.get_boot_slot()?.label,
        ota.get_running_slot()?.label,
        ota.get_update_slot()?.label
    );

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
            let mut client = Client::wrap(
                EspHttpConnection::new(&Configuration {
                    buffer_size: Some(4096),
                    ..Default::default()
                })?,
            );

            let uri_str = uri.to_string();
            let headers = [(ACCEPT.as_str(), APPLICATION_OCTET_STREAM.as_ref())];
            let res = match client.request(Method::Get, &uri_str, &headers) {
                Ok(req) => {
                    match req.submit() {
                        Ok(resp) => handle_ota_resp(resp),
                        Err(e) => {
                            error!("Failed to send request! {e:?}");
                            esp_err!(ESP_FAIL)
                        }
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

use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::State,
    http::{StatusCode, Uri},
    routing::{get, post},
    Json, Router,
};
use esp_idf_svc::{
    hal::{
        gpio::{AnyIOPin, Output, PinDriver},
        reset::restart,
    },
    nvs::{EspCustomNvs, EspCustomNvsPartition},
    ota::{EspOta, FirmwareInfo},
    sys::{EspError, ESP_ERR_IMAGE_INVALID, ESP_ERR_INVALID_RESPONSE, ESP_FAIL},
};

use log::{error, info};
use serde::{Deserialize, Serialize};
use tokio::{net::TcpListener, sync::Mutex, time::sleep};

#[macro_export]
macro_rules! esp_err {
    ($x:ident) => {
        Err(EspError::from_infallible::<$x>())
    };
}

const FIRMWARE_MAX_SIZE: u64 = 0x1f0000; // Max size of each app partition
const FIRMWARE_MIN_SIZE: u64 = size_of::<FirmwareInfo>() as u64 + 1024;

const BIND_IP: &str = "0.0.0.0";
const PORT: u16 = 80;

const NVS_NS: &str = "storage";
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
        .route("/dispense", post(dispense_single))
        .route("/calibrate", post(calibrate))
        .route("/dose", post(dose_solution))
        .route("/reboot", get(reboot))
        .route("/ota", post(handle_ota))
        .with_state(state.clone());

    let listener = TcpListener::bind(format!("{BIND_IP}:{PORT}")).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn root() -> Json<&'static str> {
    Json("Nutrient doser")
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
struct DispenseSingleReq {
    motor_idx: usize,
    ml: f64,
}

async fn dispense_single(
    State(state): State<SharedState>,
    Json(req): Json<DispenseSingleReq>,
) -> StatusCode {
    match state.lock().await.motors.get_mut(req.motor_idx) {
        Some(motor) => {
            let dispense_time = req.ml / motor.ml_per_sec;
            info!("Dispensing liquid #{} for {dispense_time}s", req.motor_idx);
            motor.run_for(Duration::from_secs_f64(dispense_time)).await;
            StatusCode::OK
        }
        None => StatusCode::BAD_REQUEST,
    }
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

async fn do_ota(uri: Uri) -> Result<(), EspError> {
    let mut resp = match reqwest::get(uri.to_string()).await {
        Ok(r) => r,
        Err(e) => {
            error!("Error requesting OTA image: {e}");
            return esp_err!(ESP_FAIL);
        }
    };

    if !resp.status().is_success() {
        error!("Unexpected HTTP response: {}", resp.status());
        return esp_err!(ESP_ERR_INVALID_RESPONSE);
    }

    // Check firmware size
    let file_size = resp.content_length().unwrap_or_default();
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
    let mut upd = ota.initiate_update()?;
    let mut written: usize = 0;
    while let Ok(Some(chunk)) = resp.chunk().await {
        upd.write(&chunk[..])?;
        written += chunk.len();
        info!(
            "OTA progress: {:.2}%",
            100.0 * written as f32 / file_size as f32
        );
    }

    // OTA was successful if we reach this
    upd.complete()
}

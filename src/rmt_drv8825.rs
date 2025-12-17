use esp_idf_svc::{
    hal::{
        gpio::{AnyOutputPin, Level, Output, PinDriver},
        rmt::{PinState, Pulse, PulseTicks, Symbol, TxRmtDriver},
        units::Hertz,
    },
    sys::EspError,
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use stepgen::Stepgen;

const MAX_STEP_FREQ: Hertz = Hertz(250000);
const STEP_PULSE: Duration = Duration::from_micros(2);
const DIR_SETUP: Duration = Duration::from_nanos(650);
const EN_SETUP: Duration = Duration::from_nanos(650);

// TODO: make this configurable for different motor models
const MAX_RPM: f64 = 400.0;
const MAX_ACCEL: f64 = 400.0;

#[derive(Copy, Clone, Debug)]
pub enum MicroSteps {
    M1 = 1,
    M2 = 2,
    M4 = 4,
    M8 = 8,
    M16 = 16,
    M32 = 32,
}

impl MicroSteps {
    fn scale(&self, val: f64) -> f64 {
        val * (*self as u32) as f64
    }
}

pub struct DRV8825 {
    pin_en: PinDriver<'static, AnyOutputPin, Output>,
    pin_dir: PinDriver<'static, AnyOutputPin, Output>,
    tx: Arc<Mutex<TxRmtDriver<'static>>>,
    clock: Hertz,
    position: i32,
    microsteps: MicroSteps,
}

impl DRV8825 {
    pub fn new(
        pin_en: AnyOutputPin,
        pin_dir: AnyOutputPin,
        tx: Arc<Mutex<TxRmtDriver<'static>>>,
        microsteps: MicroSteps,
    ) -> Result<Self, EspError> {
        let clock = tx.lock().unwrap().counter_clock()?;
        let mut en = PinDriver::output(pin_en)?;
        let dir = PinDriver::output(pin_dir)?;
        en.set_high()?; // Make sure the motor is disabled from the start

        Ok(Self {
            pin_en: en,
            pin_dir: dir,
            tx,
            clock,
            position: 0,
            microsteps,
        })
    }

    pub fn id(&self) -> u32 {
        self.pin_en.pin() as u32
    }

    fn gen_steps(&self, steps: f64) -> Result<impl Iterator<Item = Symbol>, EspError> {
        let one_step_ticks = PulseTicks::new_with_duration(self.clock, &STEP_PULSE)?;

        let mut sg = Stepgen::new(self.clock.0);
        sg.set_acceleration((self.microsteps.scale(MAX_ACCEL) as u32) << 8).unwrap();
        sg.set_target_speed((self.microsteps.scale(MAX_RPM / 60.0 * 200.0) as u32) << 8).unwrap();
        sg.set_target_step(self.microsteps.scale(steps) as u32).unwrap();

        // The generated delays are ticks between the rising edges of two pulses,
        // need to make sure the length of the high pulse is subtracted from the
        // low pulse when converting these to signals
        Ok(sg.map(move |delay| {
            Symbol::new(
                Pulse::new(PinState::High, one_step_ticks),
                Pulse::new(PinState::Low, PulseTicks::new((delay >> 8) as u16 - one_step_ticks.ticks()).expect("gen_low_pulse")),
            )
        }))
    }

    pub async fn step_by(&mut self, steps: f64) -> Result<(), EspError> {
        // Setup, then wait 650ns
        self.pin_dir.set_level(match steps {
            ..=0.0 => Level::Low,
            _ => Level::High,
        })?;
        self.pin_en.set_low()?;
        tokio::time::sleep(EN_SETUP).await;

        // Generate and send pulses to the stepper motor
        let res = match self.gen_steps(steps.abs()) {
            Ok(syms) => {
                let _tx = Arc::clone(&self.tx);
                tokio::task::spawn_blocking(move || {
                    _tx.lock().unwrap().start_iter_blocking(syms)
                }).await.expect("Failed to send pulses")
            }
            Err(e) => Err(e),
        };

        // Done, de-energize coils
        self.pin_en.set_high()?;

        self.position = self.position.saturating_add(steps.round() as i32);
        res
    }

    pub async fn goto(&mut self, target_pos: i32) -> Result<(), EspError> {
        self.step_by(target_pos.saturating_sub(self.position) as f64).await
    }

    pub fn get_position(&self) -> i32 {
        self.position
    }

    pub fn reset_position(&mut self) {
        self.position = 0;
    }
}

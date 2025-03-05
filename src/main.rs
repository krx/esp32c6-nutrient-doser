mod app;

use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{
        gpio::{AnyIOPin, PinDriver},
        prelude::Peripherals,
    },
    io::vfs::MountedEventfs,
    nvs::EspDefaultNvsPartition,
    ota::EspOta,
    sys::EspError,
    timer::EspTimerService,
    wifi::{AsyncWifi, ClientConfiguration, Configuration, EspWifi},
};
use log::{info, warn};

#[toml_cfg::toml_config]
pub struct Config {
    #[default("")]
    wifi_ssid: &'static str,
    #[default("")]
    wifi_pass: &'static str,
}

fn set_ota_valid() {
    let mut ota = EspOta::new().expect("Instantiate EspOta");
    ota.mark_running_slot_valid().expect("Mark app slot as valid");
}

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    let _event_fds = MountedEventfs::mount(5).unwrap();
    set_ota_valid();

    let peripherals = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let timer_service = EspTimerService::new()?;
    let nvs = EspDefaultNvsPartition::take()?;

    info!("Initializing Wi-Fi...");
    let wifi = AsyncWifi::wrap(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs))?,
        sys_loop,
        timer_service.clone(),
    )?;

    let motor_pins = vec![
        PinDriver::output(AnyIOPin::from(peripherals.pins.gpio21))?,
        PinDriver::output(AnyIOPin::from(peripherals.pins.gpio2))?,
        PinDriver::output(AnyIOPin::from(peripherals.pins.gpio1))?,
        PinDriver::output(AnyIOPin::from(peripherals.pins.gpio0))?,
    ];

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async move {
            // Start wifi loop first
            let mut wifi_loop = WifiLoop { wifi };
            wifi_loop.configure().await?;
            wifi_loop.initial_connect().await?;

            // Launch all other tasks
            tokio::spawn(app::run(motor_pins));

            // Keep this task on the wifi loop
            wifi_loop.stay_connected().await
        })?;

    Ok(())
}

// From https://github.com/jasta/esp32-tokio-demo
pub struct WifiLoop<'a> {
    wifi: AsyncWifi<EspWifi<'a>>,
}

impl WifiLoop<'_> {
    pub async fn configure(&mut self) -> Result<(), EspError> {
        info!("Setting Wi-Fi credentials...");
        self.wifi
            .set_configuration(&Configuration::Client(ClientConfiguration {
                ssid: CONFIG.wifi_ssid.try_into().unwrap(),
                password: CONFIG.wifi_pass.try_into().unwrap(),
                ..Default::default()
            }))?;

        info!("Starting Wi-Fi driver...");
        self.wifi.start().await
    }

    pub async fn initial_connect(&mut self) -> Result<(), EspError> {
        self.do_connect_loop(true).await
    }

    pub async fn stay_connected(mut self) -> Result<(), EspError> {
        self.do_connect_loop(false).await
    }

    async fn do_connect_loop(&mut self, exit_after_first_connect: bool) -> Result<(), EspError> {
        loop {
            // Wait for disconnect before trying to connect again.  This loop ensures
            // we stay connected and is commonly missing from trivial examples as it's
            // way too difficult to showcase the core logic of an example and have
            // a proper Wi-Fi event loop without a robust async runtime.  Fortunately, we can do it
            // now!
            self.wifi.wifi_wait(|wifi| wifi.is_up(), None).await?;

            info!("Connecting to Wi-Fi...");
            match self.wifi.connect().await {
                Ok(_) => (),
                Err(e) => {
                    warn!("Error while connecting: {e}. Retrying");
                    continue;
                }
            };

            info!("Waiting for association...");
            self.wifi
                .ip_wait_while(|wifi| wifi.is_up().map(|s| !s), None)
                .await?;

            if exit_after_first_connect {
                return Ok(());
            }
        }
    }
}

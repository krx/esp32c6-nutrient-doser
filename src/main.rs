mod app;
mod rmt_drv8825;
mod util;

use std::sync::{Arc, Mutex};

use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{
        gpio::AnyOutputPin,
        prelude::Peripherals,
        rmt::{PinState, TxRmtConfig, TxRmtDriver},
    },
    io::vfs::MountedEventfs,
    netif::{EspNetif, NetifStack},
    nvs::EspDefaultNvsPartition,
    sys::EspError,
    timer::EspTimerService,
    wifi::{AsyncWifi, ClientConfiguration, Configuration, EspWifi, WifiDriver},
};
use log::{info, warn};
use smart_leds::{brightness, colors, gamma};
use ws2812_esp32_rmt_driver::Ws2812Esp32Rmt;

use crate::rmt_drv8825::{MicroSteps, DRV8825};

#[toml_cfg::toml_config]
pub struct Config {
    #[default("")]
    wifi_ssid: &'static str,
    #[default("")]
    wifi_pass: &'static str,
}

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    let _event_fds = MountedEventfs::mount(5).unwrap();
    util::set_ota_valid();

    let peripherals = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let timer_service = EspTimerService::new()?;
    let nvs = EspDefaultNvsPartition::take()?;

    info!("Initializing Wi-Fi...");
    let wifi = AsyncWifi::wrap(
        EspWifi::wrap_all(
            WifiDriver::new(peripherals.modem, sys_loop.clone(), Some(nvs.clone()))?,
            util::get_netif_with_hostname(nvs.clone())?,
            EspNetif::new(NetifStack::Ap)?,
        )?,
        sys_loop,
        timer_service.clone(),
    )?;

    // There are a limited number of RMT TX channels available, for the esp32c6
    // there are only 2. To get around this, a single TX driver is shared across
    // all steppers, which imposes these limitations:
    // 1. Only one stepper can be run at a time due to separate calibrations,
    //    so the driver is put behind a mutex
    // 2. Only the EN pin of the motor being run should be enabled, all others disabled
    let tx_conf = TxRmtConfig::default()
        .idle(Some(PinState::Low))
        .clock_divider(40); // 80MHz -> 2MHz

    let tx = Arc::new(Mutex::new(TxRmtDriver::new(
        peripherals.rmt.channel1,
        peripherals.pins.gpio15,
        &tx_conf,
    )?));

    let drivers = vec![
        DRV8825::new(
            AnyOutputPin::from(peripherals.pins.gpio4),
            AnyOutputPin::from(peripherals.pins.gpio5),
            tx.clone(),
            MicroSteps::M32,
        )?,
        DRV8825::new(
            AnyOutputPin::from(peripherals.pins.gpio6),
            AnyOutputPin::from(peripherals.pins.gpio7),
            tx.clone(),
            MicroSteps::M32,
        )?,
        DRV8825::new(
            AnyOutputPin::from(peripherals.pins.gpio0),
            AnyOutputPin::from(peripherals.pins.gpio1),
            tx.clone(),
            MicroSteps::M32,
        )?,
        DRV8825::new(
            AnyOutputPin::from(peripherals.pins.gpio23),
            AnyOutputPin::from(peripherals.pins.gpio22),
            tx.clone(),
            MicroSteps::M32,
        )?,
        DRV8825::new(
            AnyOutputPin::from(peripherals.pins.gpio21),
            AnyOutputPin::from(peripherals.pins.gpio20),
            tx.clone(),
            MicroSteps::M32,
        )?,
    ];

    // status LED
    let user_led = Ws2812Esp32Rmt::new(peripherals.rmt.channel0, peripherals.pins.gpio8)?;

    tokio::runtime::Builder::new_current_thread()
        .thread_stack_size(6 * 1024)
        .enable_all()
        .build()?
        .block_on(async move {
            // Start wifi loop first
            let mut wifi_loop = WifiLoop { wifi, user_led };
            wifi_loop.configure().await?;
            wifi_loop.initial_connect().await?;

            // Launch all other tasks
            tokio::spawn(app::run(drivers));

            // Keep this task on the wifi loop
            wifi_loop.stay_connected().await
        })?;

    Ok(())
}

// From https://github.com/jasta/esp32-tokio-demo
pub struct WifiLoop<'a> {
    wifi: AsyncWifi<EspWifi<'a>>,
    user_led: Ws2812Esp32Rmt<'a>,
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
            self.user_led
                .write_nocopy(brightness(gamma([colors::ORANGE].into_iter()), 64))
                .unwrap();
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

            self.user_led
                .write_nocopy(brightness(gamma([colors::LIME].into_iter()), 64))
                .unwrap();
            if exit_after_first_connect {
                return Ok(());
            }
        }
    }
}

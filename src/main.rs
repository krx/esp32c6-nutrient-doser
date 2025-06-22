mod app;

use app::NVS_NS;
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{
        gpio::{AnyIOPin, Output, PinDriver},
        prelude::Peripherals,
    },
    io::vfs::MountedEventfs,
    ipv4::{
        ClientConfiguration as IpClientConfiguration,
        Configuration as IpConfiguration,
        DHCPClientSettings,
    },
    netif::{EspNetif, NetifConfiguration, NetifStack},
    nvs::{EspDefaultNvs, EspDefaultNvsPartition},
    ota::EspOta,
    sys::EspError,
    timer::EspTimerService,
    wifi::{AsyncWifi, ClientConfiguration, Configuration, EspWifi, WifiDriver},
};
use log::{info, warn};

const HOSTNAME_KEY: &str = "HOSTNAME";

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

fn get_netif_with_hostname(nvsp: EspDefaultNvsPartition) -> anyhow::Result<EspNetif> {
    let mut nvs = EspDefaultNvs::new(nvsp, NVS_NS, true)?;
    let hostname = match option_env!("HOSTNAME") {
        Some(h) => h.to_owned(),
        None => match nvs.str_len(HOSTNAME_KEY)? {
            Some(len) => {
                let mut buf = vec![0_u8; len];
                match nvs.get_str(HOSTNAME_KEY, buf.as_mut_slice())? {
                    Some(h) => h.to_owned(),
                    None => String::new(),
                }
            }
            None => String::new(),
        },
    };

    if !hostname.is_empty() {
        warn!("Setting hostname to {hostname}");
        nvs.set_str(HOSTNAME_KEY, hostname.as_str())?;

        Ok(EspNetif::new_with_conf(&NetifConfiguration {
            ip_configuration: Some(IpConfiguration::Client(IpClientConfiguration::DHCP(
                DHCPClientSettings {
                    hostname: Some(hostname.as_str().try_into().unwrap()),
                },
            ))),
            ..NetifConfiguration::wifi_default_client()
        })?)
    } else {
        Ok(EspNetif::new(NetifStack::Sta)?)
    }
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
        EspWifi::wrap_all(
            WifiDriver::new(peripherals.modem, sys_loop.clone(), Some(nvs.clone()))?, 
            get_netif_with_hostname(nvs.clone())?,
            EspNetif::new(NetifStack::Ap)?,
        )?,
        sys_loop,
        timer_service.clone(),
    )?;

    let motor_pins = vec![
        PinDriver::output(AnyIOPin::from(peripherals.pins.gpio21))?,
        PinDriver::output(AnyIOPin::from(peripherals.pins.gpio2))?,
        PinDriver::output(AnyIOPin::from(peripherals.pins.gpio1))?,
        PinDriver::output(AnyIOPin::from(peripherals.pins.gpio0))?,
    ];

    // enable external antenna
    let mut gpio3 = PinDriver::output(peripherals.pins.gpio3)?;
    gpio3.set_low()?;
    let mut gpio14 = PinDriver::output(peripherals.pins.gpio14)?;
    gpio14.set_high()?;

    // status LED
    let user_led = PinDriver::output(AnyIOPin::from(peripherals.pins.gpio15))?;

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
            tokio::spawn(app::run(motor_pins));

            // Keep this task on the wifi loop
            wifi_loop.stay_connected().await
        })?;

    Ok(())
}

// From https://github.com/jasta/esp32-tokio-demo
pub struct WifiLoop<'a> {
    wifi: AsyncWifi<EspWifi<'a>>,
    user_led: PinDriver<'a, AnyIOPin, Output>,
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
            self.user_led.set_low()?;
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

            self.user_led.set_high()?;
            if exit_after_first_connect {
                return Ok(());
            }
        }
    }
}

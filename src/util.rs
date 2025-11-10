use esp_idf_svc::{
    ipv4::{
        ClientConfiguration as IpClientConfiguration, Configuration as IpConfiguration,
        DHCPClientSettings,
    },
    netif::{EspNetif, NetifConfiguration, NetifStack},
    nvs::{EspDefaultNvs, EspDefaultNvsPartition},
    ota::EspOta,
};

use log::warn;

use crate::app::NVS_NS;

const HOSTNAME_KEY: &str = "HOSTNAME";

pub fn set_ota_valid() {
    let mut ota = EspOta::new().expect("Instantiate EspOta");
    ota.mark_running_slot_valid()
        .expect("Mark app slot as valid");
}

pub fn get_netif_with_hostname(nvsp: EspDefaultNvsPartition) -> anyhow::Result<EspNetif> {
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

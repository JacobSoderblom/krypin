use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ZigbeeInfo {
    pub ieee_address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_address: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoints: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub power_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firmware_version: Option<String>,
}

impl ZigbeeInfo {
    pub fn new(ieee_address: impl Into<String>) -> Self {
        Self {
            ieee_address: ieee_address.into(),
            network_address: None,
            endpoints: None,
            power_source: None,
            firmware_version: None,
        }
    }

    pub fn with_network_address(mut self, network_address: u16) -> Self {
        self.network_address = Some(network_address);
        self
    }

    pub fn with_endpoints(mut self, endpoints: Vec<u8>) -> Self {
        self.endpoints = Some(endpoints);
        self
    }

    pub fn with_power_source(mut self, power_source: impl Into<String>) -> Self {
        self.power_source = Some(power_source.into());
        self
    }

    pub fn with_firmware_version(mut self, firmware_version: impl Into<String>) -> Self {
        self.firmware_version = Some(firmware_version.into());
        self
    }
}

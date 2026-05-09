//! Device registry - tracks connected devices and their status.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use super::serial::{DeviceType, SerialDevice};

/// Status of a connected device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceStatus {
    Connected,
    Streaming,
    Error(String),
    Disconnected,
}

/// A registered device with its current status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredDevice {
    pub device: SerialDevice,
    pub status: DeviceStatus,
    pub patient_id: Option<String>,
    pub tenant_id: Option<String>,
    pub last_reading_at: Option<String>,
}

/// Registry of all known devices.
pub struct DeviceRegistry {
    devices: HashMap<String, RegisteredDevice>,
}

impl DeviceRegistry {
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
        }
    }

    pub fn register(&mut self, port: String, device: SerialDevice) {
        self.devices.insert(port.clone(), RegisteredDevice {
            device,
            status: DeviceStatus::Connected,
            patient_id: None,
            tenant_id: None,
            last_reading_at: None,
        });
    }

    pub fn update_status(&mut self, port: &str, status: DeviceStatus) {
        if let Some(dev) = self.devices.get_mut(port) {
            dev.status = status;
        }
    }

    pub fn assign_patient(&mut self, port: &str, patient_id: &str, tenant_id: &str) {
        if let Some(dev) = self.devices.get_mut(port) {
            dev.patient_id = Some(patient_id.to_string());
            dev.tenant_id = Some(tenant_id.to_string());
        }
    }

    pub fn list_devices(&self) -> Vec<&RegisteredDevice> {
        self.devices.values().collect()
    }

    pub fn remove(&mut self, port: &str) {
        self.devices.remove(port);
    }
}

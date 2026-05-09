//! USB Serial device enumeration and connection.
//! Supports CDC/ACM, FTDI, CP210x, CH340 adapters.

use serde::{Deserialize, Serialize};

/// Represents a discovered serial device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerialDevice {
    pub port: String,
    pub vendor_id: u16,
    pub product_id: u16,
    pub description: String,
    pub device_type: DeviceType,
}

/// Known medical device types by USB vendor/product ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceType {
    PulseOximeter,
    BloodPressure,
    ECGMonitor,
    Glucometer,
    Thermometer,
    Scale,
    Unknown,
}

/// Enumerate available serial ports and identify medical devices.
pub fn enumerate_devices() -> Vec<SerialDevice> {
    // TODO: Use serialport crate to list available ports
    // For each port, check vendor_id/product_id against known devices
    vec![]
}

/// Known device identifiers (vendor_id, product_id) → DeviceType
pub fn identify_device(vendor_id: u16, product_id: u16) -> DeviceType {
    match (vendor_id, product_id) {
        // Contec CMS50D+ (SpO2)
        (0x10C4, 0xEA60) => DeviceType::PulseOximeter,
        // Nonin 3150 (SpO2)
        (0x0403, 0x6001) => DeviceType::PulseOximeter,
        // Omron (Blood Pressure)
        (0x0590, _) => DeviceType::BloodPressure,
        // Welch Allyn
        (0x0681, _) => DeviceType::ECGMonitor,
        _ => DeviceType::Unknown,
    }
}

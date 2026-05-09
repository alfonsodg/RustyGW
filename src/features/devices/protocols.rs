//! Protocol parsers for medical device data streams.
//! Each device type has its own byte-level protocol.

use serde::{Deserialize, Serialize};

/// A vital sign reading from a device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VitalReading {
    pub vital_type: String,
    pub value: f64,
    pub unit: String,
    pub device_id: String,
    pub timestamp: String,
}

/// Parse Contec CMS50D+ pulse oximeter data.
/// Protocol: 5 bytes per packet, byte[3]=pulse, byte[4]=spo2
pub fn parse_contec_cms50d(data: &[u8]) -> Option<Vec<VitalReading>> {
    if data.len() < 5 {
        return None;
    }
    let spo2 = data[4] as f64;
    let pulse = data[3] as f64;

    if spo2 < 50.0 || spo2 > 100.0 || pulse < 30.0 || pulse > 250.0 {
        return None; // Invalid reading
    }

    Some(vec![
        VitalReading {
            vital_type: "spo2".into(),
            value: spo2,
            unit: "%".into(),
            device_id: "contec_cms50d".into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        },
        VitalReading {
            vital_type: "heart_rate".into(),
            value: pulse,
            unit: "bpm".into(),
            device_id: "contec_cms50d".into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        },
    ])
}

/// Parse generic blood pressure monitor data.
/// Protocol varies by manufacturer, this handles common format:
/// byte[1]=systolic, byte[3]=diastolic, byte[11]=pulse
pub fn parse_bp_monitor(data: &[u8]) -> Option<Vec<VitalReading>> {
    if data.len() < 12 {
        return None;
    }
    let systolic = data[1] as f64;
    let diastolic = data[3] as f64;
    let pulse = data[11] as f64;

    if systolic < 60.0 || systolic > 260.0 || diastolic < 30.0 || diastolic > 150.0 {
        return None;
    }

    Some(vec![
        VitalReading {
            vital_type: "systolic_bp".into(),
            value: systolic,
            unit: "mmHg".into(),
            device_id: "bp_monitor".into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        },
        VitalReading {
            vital_type: "diastolic_bp".into(),
            value: diastolic,
            unit: "mmHg".into(),
            device_id: "bp_monitor".into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        },
        VitalReading {
            vital_type: "heart_rate".into(),
            value: pulse,
            unit: "bpm".into(),
            device_id: "bp_monitor".into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        },
    ])
}

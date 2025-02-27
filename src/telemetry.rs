use anyhow::{anyhow, Result};

use crate::nmea::Sentence;

/**
 * GPS telemetry
 *
 * TODO: Add speed, course, altitude, etc.
 * For now as a proof of concept this is just latitude and longitude
 */
#[derive(Debug, Clone)]
pub struct Telemetry {
    pub time: String,
    pub latitude: f64,
    pub longitude: f64,
}

impl TryFrom<Sentence> for Telemetry {
    type Error = anyhow::Error;

    fn try_from(sentence: Sentence) -> Result<Self> {
        match sentence {
            // Let's just support RMC for now; GGA only adds altitude which may not be useful at this point
            Sentence::RMC {
                time,
                latitude,
                longitude,
                ..
            } => Ok(Telemetry {
                time,
                latitude,
                longitude,
            }),
            Sentence::Unknown(variant) => Err(anyhow!("Unsupported sentence variant: {}", variant)),
            _ => Err(anyhow!(
                "Sentence variant supported but not used for telemetry"
            )),
        }
    }
}

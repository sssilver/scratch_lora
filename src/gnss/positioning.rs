use crate::gnss::error::GnssError;
use chrono::NaiveDateTime;
use defmt::Format;
use nmea::sentences::rmc::RmcStatusOfFix;
use nmea::ParseResult;

#[derive(Debug, Clone, PartialEq, Format)]
pub struct GnssPositioning {
    #[defmt(Debug2Format)]
    pub datetime: NaiveDateTime,
    pub latitude: f64,
    pub longitude: f64,
    pub speed: Option<f32>,
    pub heading: Option<f32>,
}

impl TryFrom<ParseResult> for GnssPositioning {
    type Error = GnssError;

    fn try_from(data: ParseResult) -> Result<Self, Self::Error> {
        let rmc = if let ParseResult::RMC(rmc) = data {
            rmc
        } else {
            return Err(GnssError::UnsupportedSentence);
        };

        if rmc.status_of_fix == RmcStatusOfFix::Invalid {
            return Err(GnssError::NoFix);
        }

        let latitude = rmc.lat.ok_or_else(|| GnssError::MissingField("latitude"))?;
        let longitude = rmc
            .lon
            .ok_or_else(|| GnssError::MissingField("longitude"))?;
        let fix_date = rmc
            .fix_date
            .ok_or_else(|| GnssError::MissingField("fix_date"))?;
        let fix_time = rmc
            .fix_time
            .ok_or_else(|| GnssError::MissingField("fix_time"))?;

        // Convert ParsedData into SatellitePositioning
        Ok(GnssPositioning {
            datetime: fix_date.and_time(fix_time),
            latitude,
            longitude,
            speed: rmc.speed_over_ground,
            heading: rmc.true_course,
        })
    }
}

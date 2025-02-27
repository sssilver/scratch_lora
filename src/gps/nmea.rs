use core::str::FromStr;

use super::{error::GpsError, positioning::Positioning};

#[derive(Debug)]
pub enum NmeaError {
    InvalidChecksum,
    InvalidFormat,
    InvalidField,
    UnsupportedSentence,
}

#[derive(Debug)]
pub enum NmeaSentence {
    GGA(GgaSentence),
    RMC(RmcSentence),
}

#[derive(Debug)]
pub struct GgaSentence {
    pub latitude: f32,
    pub longitude: f32,
    pub fix: bool,
    pub satellites: u8,
}

#[derive(Debug)]
pub struct RmcSentence {
    pub latitude: f32,
    pub longitude: f32,
    pub valid: bool,
    pub speed_knots: f32,
}

fn parse_latitude(lat: &str, direction: &str) -> Result<f32, NmeaError> {
    if lat.is_empty() || direction.is_empty() {
        return Ok(0.0);
    }

    if lat.len() < 4 {
        return Err(NmeaError::InvalidField);
    }

    let degrees = f32::from_str(&lat[..2]).map_err(|_| NmeaError::InvalidField)?;
    let minutes = f32::from_str(&lat[2..]).map_err(|_| NmeaError::InvalidField)?;
    let mut result = degrees + (minutes / 60.0);

    if direction == "S" {
        result = -result;
    } else if direction != "N" {
        return Err(NmeaError::InvalidField);
    }

    Ok(result)
}

fn parse_longitude(lon: &str, direction: &str) -> Result<f32, NmeaError> {
    if lon.is_empty() || direction.is_empty() {
        return Ok(0.0);
    }

    if lon.len() < 5 {
        return Err(NmeaError::InvalidField);
    }

    let degrees = f32::from_str(&lon[..3]).map_err(|_| NmeaError::InvalidField)?;
    let minutes = f32::from_str(&lon[3..]).map_err(|_| NmeaError::InvalidField)?;
    let mut result = degrees + (minutes / 60.0);

    if direction == "W" {
        result = -result;
    } else if direction != "E" {
        return Err(NmeaError::InvalidField);
    }

    Ok(result)
}

fn verify_checksum(sentence: &str) -> bool {
    if let Some(asterisk_pos) = sentence.rfind('*') {
        if asterisk_pos + 3 > sentence.len() {
            return false;
        }

        let data = &sentence[1..asterisk_pos]; // Skip the $ at start
        let checksum = match u8::from_str_radix(&sentence[asterisk_pos + 1..asterisk_pos + 3], 16) {
            Ok(cs) => cs,
            Err(_) => return false,
        };

        let calculated = data.bytes().fold(0u8, |acc, b| acc ^ b);
        return calculated == checksum;
    }
    false
}

pub fn parse_sentence(sentence: &str) -> Result<NmeaSentence, NmeaError> {
    // Basic validation
    if !sentence.starts_with('$') {
        return Err(NmeaError::InvalidFormat);
    }

    // Verify checksum if present
    if sentence.contains('*') {
        if !verify_checksum(sentence) {
            return Err(NmeaError::InvalidChecksum);
        }
    } else {
        // No checksum present
        return Err(NmeaError::InvalidFormat);
    }

    // Get sentence type (first field)
    let sentence_type = sentence.split(',').next().ok_or(NmeaError::InvalidFormat)?;

    match sentence_type {
        "$GPGGA" | "$GNGGA" => parse_gga(sentence),
        "$GPRMC" | "$GNRMC" => parse_rmc(sentence),
        _ => Err(NmeaError::UnsupportedSentence),
    }
}

fn get_field<'a>(sentence: &'a str, index: usize) -> Result<&'a str, NmeaError> {
    let mut current_field = 0;
    let mut current_pos = 0;
    let mut in_field = true;

    for (pos, c) in sentence.char_indices() {
        match c {
            ',' => {
                if current_field == index {
                    return Ok(&sentence[current_pos..pos]);
                }
                current_field += 1;
                current_pos = pos + 1;
                in_field = true;
            }
            '*' => break,
            _ => {
                if in_field {
                    in_field = false;
                }
            }
        }
    }

    if current_field == index {
        if let Some(asterisk_pos) = sentence[current_pos..].find('*') {
            Ok(&sentence[current_pos..current_pos + asterisk_pos])
        } else {
            Ok(&sentence[current_pos..]) // No asterisk found
        }
    } else {
        Err(NmeaError::InvalidFormat)
    }
}

fn parse_gga(sentence: &str) -> Result<NmeaSentence, NmeaError> {
    // Fields: $GPGGA,time,lat,N/S,lon,E/W,fix,sats,hdop,alt,M,geoid,M,age,ref*cs
    let fix = match get_field(sentence, 6)? {
        "0" => false,
        "1" | "2" | "3" | "4" | "5" | "6" => true,
        "" => false,
        _ => return Err(NmeaError::InvalidField),
    };

    let latitude = if let (Ok(lat), Ok(dir)) = (get_field(sentence, 2), get_field(sentence, 3)) {
        parse_latitude(lat, dir)?
    } else {
        0.0
    };

    let longitude = if let (Ok(lon), Ok(dir)) = (get_field(sentence, 4), get_field(sentence, 5)) {
        parse_longitude(lon, dir)?
    } else {
        0.0
    };

    let satellites = if let Ok(sats) = get_field(sentence, 7) {
        if sats.is_empty() {
            0
        } else {
            u8::from_str(sats).map_err(|_| NmeaError::InvalidField)?
        }
    } else {
        0
    };

    Ok(NmeaSentence::GGA(GgaSentence {
        latitude,
        longitude,
        fix,
        satellites,
    }))
}

fn parse_rmc(sentence: &str) -> Result<NmeaSentence, NmeaError> {
    // Fields: $GPRMC,time,status,lat,N/S,lon,E/W,speed,course,date,mag,dir,mode*cs
    let valid = match get_field(sentence, 2)? {
        "A" => true,
        "V" | "" => false,
        _ => return Err(NmeaError::InvalidField),
    };

    let latitude = if let (Ok(lat), Ok(dir)) = (get_field(sentence, 3), get_field(sentence, 4)) {
        parse_latitude(lat, dir)?
    } else {
        0.0
    };

    let longitude = if let (Ok(lon), Ok(dir)) = (get_field(sentence, 5), get_field(sentence, 6)) {
        parse_longitude(lon, dir)?
    } else {
        0.0
    };

    let speed_knots = if let Ok(speed) = get_field(sentence, 7) {
        if speed.is_empty() {
            0.0
        } else {
            f32::from_str(speed).map_err(|_| NmeaError::InvalidField)?
        }
    } else {
        0.0
    };

    Ok(NmeaSentence::RMC(RmcSentence {
        latitude,
        longitude,
        valid,
        speed_knots,
    }))
}

impl TryFrom<NmeaSentence> for Positioning {
    type Error = GpsError;

    fn try_from(sentence: NmeaSentence) -> Result<Self, Self::Error> {
        match sentence {
            NmeaSentence::GGA(gga) => {
                if !gga.fix {
                    return Err(GpsError::NoFix);
                }

                Ok(Positioning {
                    latitude: gga.latitude,
                    longitude: gga.longitude,
                })
            }
            NmeaSentence::RMC(rmc) => {
                if !rmc.valid {
                    return Err(GpsError::NoFix);
                }

                Ok(Positioning {
                    latitude: rmc.latitude,
                    longitude: rmc.longitude,
                })
            }
        }
    }
}

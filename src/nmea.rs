/**
 * NMEA Sentence Parser
 *
 * NOTE: This parser was implemented by hand because the `nmea` crate panicked
 * when the parser was initialized with .default().
 *
 * However, since then, https://github.com/AeroRust/nmea/issues/143 revealed that
 * simply using the .parse_() would work with much lower resource requirements.
 *
 * Thus, it makes sense at some point to consider switching to the `nmea` crate.
 */
use std::str::FromStr;

use anyhow::{anyhow, Context, Result};

#[derive(Debug)]
pub enum Sentence {
    /// GGA - Global Positioning System Fix Data
    /// Provides essential fix data including 3D location and accuracy
    GGA {
        /// UTC time in HHMMSS.SS format
        time: String,
        /// Latitude in decimal degrees
        latitude: f64,
        /// Latitude direction: 'N' for North or 'S' for South
        latitude_dir: char,
        /// Longitude in decimal degrees
        longitude: f64,
        /// Longitude direction: 'E' for East or 'W' for West
        longitude_dir: char,
        /// GPS Quality indicator:
        /// * 0 = Invalid
        /// * 1 = GPS fix (SPS)
        /// * 2 = DGPS fix
        /// * 3 = PPS fix
        /// * 4 = Real Time Kinematic
        /// * 5 = Float RTK
        /// * 6 = Estimated (dead reckoning)
        /// * 7 = Manual input mode
        /// * 8 = Simulation mode
        fix_quality: u8,
        /// Number of satellites being tracked
        num_satellites: u8,
        /// Horizontal dilution of position (HDOP)
        hdop: f64,
        /// Altitude above mean sea level
        altitude: f64,
        /// Altitude units ('M' for meters)
        altitude_units: char,
        /// Height of geoid above WGS84 ellipsoid
        geoidal_separation: f64,
        /// Geoidal separation units ('M' for meters)
        geoidal_units: char,
    },
    /// RMC - Recommended Minimum Navigation Information
    /// Contains the essential GPS data for navigation
    RMC {
        /// UTC time in HHMMSS.SS format
        time: String,
        /// Status: 'A' for active/valid, 'V' for void/invalid
        status: char,
        /// Latitude in decimal degrees
        latitude: f64,
        /// Latitude direction: 'N' for North or 'S' for South
        latitude_dir: char,
        /// Longitude in decimal degrees
        longitude: f64,
        /// Longitude direction: 'E' for East or 'W' for West
        longitude_dir: char,
        /// Speed over ground in knots
        speed: f64,
        /// Track angle in degrees (true north)
        course: Option<f64>,
        /// Date in DDMMYY format
        date: String,
        /// Mode indicator:
        /// * 'A' = Autonomous
        /// * 'D' = Differential
        /// * 'E' = Estimated
        /// * 'M' = Manual input
        /// * 'N' = Data not valid
        /// * 'S' = Simulator
        mode: Option<char>,
    },
    Unknown(String), // For unsupported or unrecognized sentences
}

fn parse_field<T: FromStr>(data: &[u8], context: &str) -> Result<T>
where
    T::Err: std::fmt::Debug,
{
    std::str::from_utf8(data)
        .context("Invalid ASCII data")? // Validate UTF-8
        .parse::<T>() // Attempt to parse into the target type
        .map_err(|e| anyhow!("{:?}", e)) // Convert FromStr::Err to anyhow::Error
        .context(context.to_owned()) // Attach custom error context
}
pub fn parse_sentence(sentence: &[u8]) -> Result<Sentence> {
    // Ensure sentence integrity
    validate_checksum(sentence)?;

    // Find the position of '*', marking the end of the main sentence
    let without_checksum_end = sentence
        .iter()
        .position(|&b| b == b'*')
        .ok_or_else(|| anyhow!("No '*' found in sentence"))?;
    let without_checksum = &sentence[1..without_checksum_end]; // Exclude '$' and checksum

    // Split the sentence into fields based on ','
    let fields: Vec<&[u8]> = without_checksum.split(|&b| b == b',').collect();

    match fields.get(0) {
        Some(&b"GPGGA") => {
            if fields.len() < 15 {
                return Err(anyhow!("Incomplete GGA sentence"));
            }
            Ok(Sentence::GGA {
                time: String::from_utf8_lossy(fields[1]).into_owned(),
                latitude: parse_latitude(fields[2], fields[3])?,
                latitude_dir: fields[3][0] as char,
                longitude: parse_longitude(fields[4], fields[5])?,
                longitude_dir: fields[5][0] as char,
                fix_quality: parse_field(fields[6], "fix_quality").unwrap_or(0),
                num_satellites: parse_field(fields[7], "num_satellites").unwrap_or(0),
                hdop: parse_field(fields[8], "hdop").unwrap_or(0.0),
                altitude: parse_field(fields[9], "altitude").unwrap_or(0.0),
                altitude_units: fields[10][0] as char,
                geoidal_separation: parse_field(fields[11], "geoidal_separation").unwrap_or(0.0),
                geoidal_units: fields[12][0] as char,
            })
        }
        Some(&b"GPRMC") => {
            if fields.len() < 12 {
                return Err(anyhow!("Incomplete RMC sentence"));
            }
            Ok(Sentence::RMC {
                time: String::from_utf8_lossy(fields[1]).into_owned(),
                status: fields[2][0] as char,
                latitude: parse_latitude(fields[3], fields[4])?,
                latitude_dir: fields[4][0] as char,
                longitude: parse_longitude(fields[5], fields[6])?,
                longitude_dir: fields[6][0] as char,
                speed: parse_field(fields[7], "speed").unwrap_or(0.0),
                course: parse_field(fields[8], "course").ok(),
                date: String::from_utf8_lossy(fields[9]).into_owned(),
                mode: fields.get(12).and_then(|s| s.first().map(|&b| b as char)),
            })
        }
        _ => Ok(Sentence::Unknown(
            String::from_utf8_lossy(fields[0]).into_owned(),
        )),
    }
}

fn parse_latitude(lat: &[u8], dir: &[u8]) -> Result<f64> {
    // Extract degrees (first two bytes) and parse as a string
    let degrees = std::str::from_utf8(&lat[..2])
        .context("Invalid UTF-8 in latitude degrees")?
        .parse::<f64>()
        .context("Invalid latitude degrees")?;

    // Extract minutes (remaining bytes) and parse as a string
    let minutes = std::str::from_utf8(&lat[2..])
        .context("Invalid UTF-8 in latitude minutes")?
        .parse::<f64>()
        .context("Invalid latitude minutes")?;

    let value = degrees + (minutes / 60.0);

    // Match direction
    match dir {
        b"N" => Ok(value),
        b"S" => Ok(-value),
        _ => Err(anyhow!("Invalid latitude direction")),
    }
}

fn parse_longitude(lon: &[u8], dir: &[u8]) -> Result<f64> {
    // Extract degrees (first three bytes) and parse as a string
    let degrees = std::str::from_utf8(&lon[..3])
        .context("Invalid UTF-8 in longitude degrees")?
        .parse::<f64>()
        .context("Invalid longitude degrees")?;

    // Extract minutes (remaining bytes) and parse as a string
    let minutes = std::str::from_utf8(&lon[3..])
        .context("Invalid UTF-8 in longitude minutes")?
        .parse::<f64>()
        .context("Invalid longitude minutes")?;

    let value = degrees + (minutes / 60.0);

    // Match direction
    match dir {
        b"E" => Ok(value),
        b"W" => Ok(-value),
        _ => Err(anyhow!("Invalid longitude direction")),
    }
}
/// Validates the checksum of an NMEA sentence.
///
/// # Arguments
/// * `sentence` - The NMEA sentence to validate including the checksum
///
/// # Returns
/// * `Ok(bool)` - True if the checksum is valid, false otherwise
/// * `Err(NmeaError)` - If the sentence format is invalid or checksum cannot be parsed
///
/// # Examples
/// ```
/// let sentence = "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47";
/// assert!(validate_checksum(sentence).unwrap());
/// ```
pub fn validate_checksum(sentence: &[u8]) -> Result<bool> {
    if sentence.is_empty() || sentence[0] != b'$' {
        return Err(anyhow!("NMEA sentence must start with '$'"));
    }

    // Find the position of checksum separator '*'
    let asterisk_pos = sentence
        .iter()
        .position(|&b| b == b'*')
        .ok_or_else(|| anyhow!("No checksum separator '*' found"))?;

    if asterisk_pos + 3 > sentence.len() {
        return Err(anyhow!("Invalid checksum length"));
    }

    let mut checksum = 0u8;
    // Calculate checksum from after '$' to before '*'
    for &byte in &sentence[1..asterisk_pos] {
        checksum ^= byte;
    }

    // Parse the two hex digits after '*'
    let high = hex_to_u8(sentence[asterisk_pos + 1])?;
    let low = hex_to_u8(sentence[asterisk_pos + 2])?;
    let provided_checksum = (high << 4) | low;

    Ok(checksum == provided_checksum)
}

fn hex_to_u8(byte: u8) -> Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(anyhow!("Invalid hex digit")),
    }
}

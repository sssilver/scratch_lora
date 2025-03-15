use defmt::Format;

#[derive(Debug, Format)]
pub enum GnssError {
    NoFix,
    MissingField(&'static str), // Specify which field is missing
    UnsupportedSentence,        // We only need to support GPRMC for now
    UartError,
    InvalidUtf8,
    ParseError,
}

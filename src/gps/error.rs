#[derive(Debug)]
pub enum GpsError {
    NoFix,
    UartError,
    Timeout,
}

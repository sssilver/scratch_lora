#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum Error {
    AdvertisementError,
    RssiReadFailed,
    GattError,
}

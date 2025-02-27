use trouble_host::prelude::gatt_service;

use super::config::DEVICE_SERVICE_UUID;

#[gatt_service(uuid = DEVICE_SERVICE_UUID)]
pub struct DeviceService {
    #[characteristic(uuid = "17a8a05b-5da4-44ae-82a5-6d660b08cf13", read, notify)]
    pub status: u8,

    #[characteristic(uuid = "17a8a05b-5da4-44ae-82a5-6d660b08cf15", read, notify)]
    pub telemetry: [u8; 24],

    #[characteristic(uuid = "17a8a05b-5da4-44ae-82a5-6d660b08cf14", read, notify)]
    pub error_log: [u8; 7],
}

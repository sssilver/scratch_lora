use bt_hci::param::{AddrKind, BdAddr};
use trouble_host::{Address, HostResources};

pub const DEVICE_SERVICE_UUID: u128 = 0x17ada41d_b564_4a77_ad1a_22cf554002fc;

const L2CAP_MTU: usize = 255;
const CONNECTIONS_MAX: usize = 1;
const L2CAP_CHANNELS_MAX: usize = 2;

pub type Resources = HostResources<CONNECTIONS_MAX, L2CAP_CHANNELS_MAX, L2CAP_MTU>;

pub struct Config {
    /// Name of the BLE device
    pub name: &'static str,

    /// Public address of the BLE device
    pub address: Address,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            name: "Small Black Box",
            address: Address {
                kind: AddrKind::PUBLIC,
                addr: BdAddr::new([0x48, 0xca, 0x43, 0x3b, 0x0f, 0xa8]),
            },
        }
    }
}

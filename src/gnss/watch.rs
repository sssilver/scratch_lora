use crate::gnss::positioning::GnssPositioning;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Watch;

pub const WATCH_BUFFER_SIZE: usize = 4;

// Static channel for latest positioning data
pub static GNSS_WATCH: Watch<CriticalSectionRawMutex, Option<GnssPositioning>, WATCH_BUFFER_SIZE> =
    Watch::new();

pub type GnssStateRx = embassy_sync::watch::Receiver<
    'static,
    CriticalSectionRawMutex,
    Option<GnssPositioning>,
    WATCH_BUFFER_SIZE,
>;

pub type GnssStateTx = embassy_sync::watch::Sender<
    'static,
    CriticalSectionRawMutex,
    Option<GnssPositioning>,
    WATCH_BUFFER_SIZE,
>;

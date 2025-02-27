use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, watch::Watch};

const WATCH_BUFFER_SIZE: usize = 4;

/// Global watch channel to track BLE state updates
pub static BLE_STATE: Watch<CriticalSectionRawMutex, State, WATCH_BUFFER_SIZE> = Watch::new();

pub type BleStateRx =
    embassy_sync::watch::Receiver<'static, CriticalSectionRawMutex, State, WATCH_BUFFER_SIZE>;

type BleStateTx =
    embassy_sync::watch::Sender<'static, CriticalSectionRawMutex, State, WATCH_BUFFER_SIZE>;

/// Current state of the BLE connection
#[derive(Clone, Debug)]
pub struct State {
    /// Indicates whether a BLE connection is active
    pub connection_status: bool,

    /// Received Signal Strength Indicator (RSSI), if available
    pub rssi: Option<i8>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            connection_status: false,
            rssi: None,
        }
    }
}

pub struct StateController {
    state: State,
    sender: BleStateTx,
}

impl StateController {
    pub fn new() -> Self {
        let state = State::default();
        let sender = BLE_STATE.sender();
        sender.send(state.clone());

        Self { state, sender }
    }

    pub fn set_connected(&mut self) {
        self.state.connection_status = true;
        self.sender.send(self.state.clone());
    }

    pub fn set_disconnected(&mut self) {
        self.state.connection_status = false;
        self.state.rssi = None;
        self.sender.send(self.state.clone());
    }
}

use crate::serial::TemperatureData;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Default)]
struct InnerState {
    temperatures: TemperatureData,
    connected: bool,
}

#[derive(Debug, Clone, Default)]
pub struct TemperatureState {
    inner: Arc<RwLock<InnerState>>,
}

impl TemperatureState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&self, data: TemperatureData) {
        if let Ok(mut state) = self.inner.write() {
            state.temperatures = data;
        }
    }

    pub fn set_connected(&self, connected: bool) {
        if let Ok(mut state) = self.inner.write() {
            state.connected = connected;
        }
    }

    pub fn get_temperatures(&self) -> [f64; 4] {
        self.inner
            .read()
            .map(|s| s.temperatures.temps)
            .unwrap_or_default()
    }

    pub fn is_connected(&self) -> bool {
        self.inner.read().map(|s| s.connected).unwrap_or(false)
    }
}

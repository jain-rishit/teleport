use crate::domain::error::Result;
use crate::domain::models::Device;
use crate::infrastructure::mtp::UsbMonitor;
use std::sync::{Arc, Mutex};

pub struct DeviceManager {
    usb_monitor: UsbMonitor,
    connected_devices: Arc<Mutex<Vec<Device>>>,
}

impl DeviceManager {
    pub fn new() -> Result<Self> {
        let usb_monitor = UsbMonitor::new()?;
        Ok(Self {
            usb_monitor,
            connected_devices: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub fn refresh_devices(&self) -> Result<Vec<Device>> {
        let devices = self.usb_monitor.list_devices()?;
        let mut connected = self.connected_devices.lock().unwrap();
        *connected = devices.clone();
        Ok(devices)
    }
}

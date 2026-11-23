use rusb::{Context, Device, DeviceDescriptor, UsbContext};
use crate::error::Result;
use crate::models::Device as AppDevice;

pub struct UsbMonitor {
    context: Context,
}

impl UsbMonitor {
    pub fn new() -> Result<Self> {
        let context = Context::new()?;
        Ok(Self { context })
    }

    pub fn list_devices(&self) -> Result<Vec<AppDevice>> {
        let devices = self.context.devices()?;
        let mut app_devices = Vec::new();

        for device in devices.iter() {
            let device_desc = device.device_descriptor()?;
            
            if let Some(app_device) = self.parse_device(&device, &device_desc) {
                app_devices.push(app_device);
            }
        }

        Ok(app_devices)
    }

    fn parse_device(&self, device: &Device<Context>, desc: &DeviceDescriptor) -> Option<AppDevice> {
        let handle = device.open().ok()?;
        let manufacturer = handle.read_manufacturer_string_ascii(desc).unwrap_or_else(|_| "Unknown".to_string());
        let product = handle.read_product_string_ascii(desc).unwrap_or_else(|_| "Unknown Device".to_string());
        let serial = handle.read_serial_number_string_ascii(desc).ok();

        let id = format!("{}-{}-{:04x}:{:04x}", 
            device.bus_number(),
            device.address(),
            desc.vendor_id(), 
            desc.product_id()
        );
        
        let mut app_device = AppDevice::new(
            id,
            product,
            manufacturer,
            desc.vendor_id(),
            desc.product_id(),
        );
        app_device.serial_number = serial;

        Some(app_device)
    }
}

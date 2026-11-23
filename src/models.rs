use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub manufacturer: String,
    pub serial_number: Option<String>,
    pub vendor_id: u16,
    pub product_id: u16,
    pub is_connected: bool,
}

impl Device {
    pub fn new(id: String, name: String, manufacturer: String, vendor_id: u16, product_id: u16) -> Self {
        Self {
            id,
            name,
            manufacturer,
            serial_number: None,
            vendor_id,
            product_id,
            is_connected: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileType {
    File,
    Directory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub file_type: FileType,
    pub modified: Option<DateTime<Utc>>,
    pub is_remote: bool,
    pub storage_id: Option<u32>,
    pub object_id: Option<u32>,
}

impl FileEntry {
    pub fn is_dir(&self) -> bool {
        matches!(self.file_type, FileType::Directory)
    }
}

#[derive(Debug, Clone)]
pub struct TransferProgress {
    #[allow(dead_code)]
    pub file_name: String,
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub started_at: std::time::Instant,
}

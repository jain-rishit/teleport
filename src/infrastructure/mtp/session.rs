use std::time::Duration;

use chrono::{DateTime, NaiveDateTime, Utc};
use rusb::{Context, Device, DeviceHandle, Direction, TransferType, UsbContext};

use crate::domain::error::{AppError, Result};
use crate::domain::models::{FileEntry, FileType};
use crate::infrastructure::mtp::protocol::{ContainerType, OpCode, ResponseCode};

const ROOT_PARENT: u32 = 0xFFFF_FFFF;
const UNDEFINED_OBJECT_FORMAT: u16 = 0x3000;
const ASSOCIATION_FORMAT: u16 = 0x3001;
const GENERIC_FOLDER_ASSOCIATION: u16 = 0x0001;
const OBJECT_FILE_NAME_PROP: u32 = 0xDC07;
const OBJECT_SIZE_PROP: u32 = 0xDC87;
const LARGE_OBJECT_MARKER: u32 = u32::MAX;
const USB_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
pub struct StorageEntry {
    pub id: u32,
    pub description: String,
}

#[derive(Debug, Clone)]
struct ObjectInfo {
    handle: u32,
    storage_id: u32,
    name: String,
    size: u64,
    format: u16,
    modified: Option<DateTime<Utc>>,
}

impl ObjectInfo {
    fn is_dir(&self) -> bool {
        self.format == ASSOCIATION_FORMAT
    }
}

pub struct MtpSession {
    handle: DeviceHandle<Context>,
    interface_number: u8,
    bulk_in: u8,
    bulk_out: u8,
    transaction_id: u32,
}

impl MtpSession {
    pub fn open(device_id: &str) -> Result<Self> {
        let context = Context::new()?;
        let devices = context.devices()?;

        for device in devices.iter() {
            let descriptor = device.device_descriptor()?;
            let candidate_id = format!(
                "{}-{}-{:04x}:{:04x}",
                device.bus_number(),
                device.address(),
                descriptor.vendor_id(),
                descriptor.product_id()
            );

            if candidate_id != device_id {
                continue;
            }

            return Self::open_device(device);
        }

        Err(AppError::Mtp(format!(
            "Device {device_id} is no longer connected"
        )))
    }

    fn open_device(device: Device<Context>) -> Result<Self> {
        let config = device.active_config_descriptor()?;
        let mut mtp_interface = None;

        'outer: for interface in config.interfaces() {
            for descriptor in interface.descriptors() {
                let is_still_image = descriptor.class_code() == 0x06
                    && descriptor.sub_class_code() == 0x01
                    && descriptor.protocol_code() == 0x01;
                let is_vendor_mtp = descriptor.class_code() == 0xFF;

                if !is_still_image && !is_vendor_mtp {
                    continue;
                }

                let mut bulk_in = None;
                let mut bulk_out = None;

                for endpoint in descriptor.endpoint_descriptors() {
                    if endpoint.transfer_type() != TransferType::Bulk {
                        continue;
                    }

                    match endpoint.direction() {
                        Direction::In => bulk_in = Some(endpoint.address()),
                        Direction::Out => bulk_out = Some(endpoint.address()),
                    }
                }

                if let (Some(bulk_in), Some(bulk_out)) = (bulk_in, bulk_out) {
                    mtp_interface = Some((descriptor.interface_number(), bulk_in, bulk_out));
                    break 'outer;
                }
            }
        }

        let (interface_number, bulk_in, bulk_out) = mtp_interface.ok_or_else(|| {
            AppError::Mtp("No MTP/PTP bulk interface found on device".to_string())
        })?;

        let handle = device.open()?;
        if handle
            .kernel_driver_active(interface_number)
            .unwrap_or(false)
        {
            let _ = handle.detach_kernel_driver(interface_number);
        }
        handle.claim_interface(interface_number)?;

        let mut session = Self {
            handle,
            interface_number,
            bulk_in,
            bulk_out,
            transaction_id: 1,
        };
        session.command(OpCode::OpenSession, &[1])?;
        Ok(session)
    }

    pub fn list_storages(&mut self) -> Result<Vec<StorageEntry>> {
        let data = self.command_with_data_in(OpCode::GetStorageIDs, &[])?;
        let ids = parse_u32_array(&data)?;
        let mut storages = Vec::new();

        for id in ids {
            let data = self.command_with_data_in(OpCode::GetStorageInfo, &[id])?;
            storages.push(StorageEntry {
                id,
                description: parse_storage_description(&data)
                    .unwrap_or_else(|| format!("Storage {:08x}", id)),
            });
        }

        Ok(storages)
    }

    pub fn list_files(
        &mut self,
        storage_id: u32,
        parent: u32,
        parent_path: &str,
    ) -> Result<Vec<FileEntry>> {
        let handles = self.object_handles(storage_id, parent)?;
        let mut files = Vec::new();

        for handle in handles {
            let info = self.object_info(handle)?;
            let is_dir = info.is_dir();
            let path = join_remote_path(parent_path, &info.name);
            files.push(FileEntry {
                name: info.name,
                path,
                size: info.size,
                file_type: if is_dir {
                    FileType::Directory
                } else {
                    FileType::File
                },
                modified: info.modified,
                is_remote: true,
                storage_id: Some(info.storage_id),
                object_id: Some(info.handle),
            });
        }

        files.sort_by(|a, b| {
            b.is_dir()
                .cmp(&a.is_dir())
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(files)
    }

    pub fn resolve_path(&mut self, path: &str) -> Result<Option<(u32, Option<u32>, String)>> {
        let normalized = normalize_remote_path(path);
        if normalized == "/" {
            return Ok(None);
        }

        let parts: Vec<&str> = normalized.trim_matches('/').split('/').collect();
        let Some((storage_name, rest)) = parts.split_first() else {
            return Ok(None);
        };

        let storage = self
            .list_storages()?
            .into_iter()
            .find(|storage| storage.description == *storage_name)
            .ok_or_else(|| AppError::Mtp(format!("Storage '{storage_name}' not found")))?;

        let mut parent = ROOT_PARENT;
        let mut current_path = format!("/{}", storage.description);

        if rest.is_empty() {
            return Ok(Some((storage.id, None, current_path)));
        }

        for part in rest {
            let child = self
                .list_object_infos(storage.id, parent)?
                .into_iter()
                .find(|info| info.name == *part)
                .ok_or_else(|| AppError::Mtp(format!("Remote path '{path}' not found")))?;

            current_path = join_remote_path(&current_path, &child.name);
            parent = child.handle;
        }

        Ok(Some((storage.id, Some(parent), current_path)))
    }

    pub fn download_object_to_file<F>(
        &mut self,
        object_id: u32,
        dest: &std::path::Path,
        mut on_progress: F,
    ) -> Result<()>
    where
        F: FnMut(u64, u64),
    {
        let info = self.object_info(object_id)?;
        let total_size = info.size;

        let transaction_id = self.next_transaction_id();
        self.write_container(
            ContainerType::Command,
            OpCode::GetObject as u16,
            transaction_id,
            &[object_id],
            &[],
        )?;

        let mut buffer = vec![0; 1024 * 1024]; // 1MB buffer for fast USB reads
        let size = self
            .handle
            .read_bulk(self.bulk_in, &mut buffer, USB_TIMEOUT)?;

        if size < 12 {
            return Err(AppError::Mtp("Short MTP packet".to_string()));
        }

        let packet_length = read_u32(&buffer, 0)? as u64;
        let container_type = match read_u16(&buffer, 4)? {
            1 => ContainerType::Command,
            2 => ContainerType::Data,
            3 => ContainerType::Response,
            4 => ContainerType::Event,
            other => {
                return Err(AppError::Mtp(format!("Unknown MTP container type {other}")));
            }
        };

        if container_type != ContainerType::Data || read_u32(&buffer, 8)? != transaction_id {
            return Err(AppError::Mtp("Expected data container".to_string()));
        }

        let mut file = std::fs::File::create(dest)
            .map_err(|e| AppError::Mtp(format!("Failed to create file: {}", e)))?;

        use std::io::Write;

        // Write the payload part of the first packet
        let payload_start = 12;
        let mut bytes_written = (size - payload_start) as u64;
        file.write_all(&buffer[payload_start..size])
            .map_err(|e| AppError::Mtp(format!("Write error: {}", e)))?;

        on_progress(bytes_written, total_size);

        // Calculate expected data length. If > 4GB, packet_length might be 0xFFFFFFFF
        let expected_data_len = if packet_length == LARGE_OBJECT_MARKER as u64 {
            total_size
        } else {
            packet_length
                .checked_sub(12)
                .ok_or_else(|| AppError::Mtp("Invalid MTP data container length".to_string()))?
        };

        if bytes_written > expected_data_len {
            return Err(AppError::Mtp(
                "MTP data container exceeds its declared size".to_string(),
            ));
        }

        while bytes_written < expected_data_len {
            let size = self
                .handle
                .read_bulk(self.bulk_in, &mut buffer, USB_TIMEOUT)?;
            if size == 0 {
                break;
            }

            let write_size = std::cmp::min(size as u64, expected_data_len - bytes_written) as usize;
            file.write_all(&buffer[..write_size])
                .map_err(|e| AppError::Mtp(format!("Write error: {}", e)))?;
            bytes_written += write_size as u64;

            on_progress(bytes_written, total_size);
        }

        if bytes_written != expected_data_len {
            return Err(AppError::Mtp(format!(
                "Incomplete MTP download: received {bytes_written} of {expected_data_len} bytes"
            )));
        }

        let response = self.read_container()?;
        ensure_response(response, transaction_id)?;
        Ok(())
    }

    pub fn delete_object(&mut self, object_id: u32) -> Result<()> {
        self.command(OpCode::DeleteObject, &[object_id, 0])?;
        Ok(())
    }

    pub fn rename_object(&mut self, object_id: u32, new_name: &str) -> Result<()> {
        let payload = encode_mtp_string(new_name);
        self.command_with_data_out(
            OpCode::SetObjectPropValue,
            &[object_id, OBJECT_FILE_NAME_PROP],
            &payload,
        )
    }

    pub fn upload_object_streaming<F>(
        &mut self,
        parent_path: &str,
        name: &str,
        src: &std::path::Path,
        mut on_progress: F,
    ) -> Result<()>
    where
        F: FnMut(u64, u64),
    {
        let (storage_id, parent) = self.resolve_parent(parent_path)?;

        let file = std::fs::File::open(src)
            .map_err(|e| AppError::Mtp(format!("Failed to open file: {}", e)))?;
        let metadata = file
            .metadata()
            .map_err(|e| AppError::Mtp(format!("Failed to read metadata: {}", e)))?;
        let total_size = metadata.len();

        let object_info = encode_object_info(
            storage_id,
            parent,
            UNDEFINED_OBJECT_FORMAT,
            0,
            mtp_size_field(total_size),
            name,
        );
        self.command_with_data_out_response(
            OpCode::SendObjectInfo,
            &[storage_id, parent],
            &object_info,
        )?;

        // Now stream SendObject
        let transaction_id = self.next_transaction_id();
        self.write_container(
            ContainerType::Command,
            OpCode::SendObject as u16,
            transaction_id,
            &[],
            &[],
        )?;

        // Send Data container header
        let length = total_size
            .checked_add(12)
            .ok_or_else(|| AppError::Mtp("File is too large for MTP transfer".to_string()))?;
        let mut header = Vec::with_capacity(12);
        header.extend_from_slice(&mtp_container_length_field(length).to_le_bytes());
        header.extend_from_slice(&(ContainerType::Data as u16).to_le_bytes());
        header.extend_from_slice(&(OpCode::SendObject as u16).to_le_bytes());
        header.extend_from_slice(&transaction_id.to_le_bytes());

        self.write_bulk_all(&header)?;

        use std::io::Read;
        let mut reader = std::io::BufReader::with_capacity(1024 * 1024, file);
        let mut buffer = vec![0; 1024 * 1024]; // 1MB chunks
        let mut bytes_written = 0;

        while bytes_written < total_size {
            let read_size = reader
                .read(&mut buffer)
                .map_err(|e| AppError::Mtp(format!("Read error: {}", e)))?;
            if read_size == 0 {
                break;
            }

            self.write_bulk_all(&buffer[..read_size])?;
            bytes_written += read_size as u64;
            on_progress(bytes_written, total_size);
        }

        if bytes_written != total_size {
            return Err(AppError::Mtp(format!(
                "Local file changed during upload: read {bytes_written} of {total_size} bytes"
            )));
        }

        let response = self.read_container()?;
        ensure_response(response, transaction_id)?;
        Ok(())
    }

    pub fn create_folder(&mut self, parent_path: &str, name: &str) -> Result<()> {
        let (storage_id, parent) = self.resolve_parent(parent_path)?;
        let object_info = encode_object_info(
            storage_id,
            parent,
            ASSOCIATION_FORMAT,
            GENERIC_FOLDER_ASSOCIATION,
            0,
            name,
        );
        self.command_with_data_out_response(
            OpCode::SendObjectInfo,
            &[storage_id, parent],
            &object_info,
        )?;
        Ok(())
    }

    fn resolve_parent(&mut self, parent_path: &str) -> Result<(u32, u32)> {
        let Some((storage_id, object_id, _)) = self.resolve_path(parent_path)? else {
            return Err(AppError::Mtp(
                "Open a device storage before writing files".to_string(),
            ));
        };

        Ok((storage_id, object_id.unwrap_or(ROOT_PARENT)))
    }

    fn list_object_infos(&mut self, storage_id: u32, parent: u32) -> Result<Vec<ObjectInfo>> {
        let handles = self.object_handles(storage_id, parent)?;
        handles
            .into_iter()
            .map(|handle| self.object_info(handle))
            .collect()
    }

    fn object_handles(&mut self, storage_id: u32, parent: u32) -> Result<Vec<u32>> {
        let data = self.command_with_data_in(OpCode::GetObjectHandles, &[storage_id, 0, parent])?;
        parse_u32_array(&data)
    }

    fn object_info(&mut self, handle: u32) -> Result<ObjectInfo> {
        let data = self.command_with_data_in(OpCode::GetObjectInfo, &[handle])?;
        let mut info = parse_object_info(handle, &data)?;
        if info.size == LARGE_OBJECT_MARKER as u64 {
            info.size = self.object_size(handle)?;
        }
        Ok(info)
    }

    /// ObjectInfo uses a 32-bit size marker for large objects; ObjectSize is UINT64.
    fn object_size(&mut self, handle: u32) -> Result<u64> {
        let data =
            self.command_with_data_in(OpCode::GetObjectPropValue, &[handle, OBJECT_SIZE_PROP])?;
        let bytes = data.get(..8).ok_or_else(|| {
            AppError::Mtp(
                "Device did not provide a 64-bit ObjectSize for a large object".to_string(),
            )
        })?;
        Ok(u64::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn command(&mut self, op: OpCode, params: &[u32]) -> Result<Vec<u32>> {
        let transaction_id = self.next_transaction_id();
        self.write_container(
            ContainerType::Command,
            op as u16,
            transaction_id,
            params,
            &[],
        )?;
        let response = self.read_container()?;
        ensure_response(response, transaction_id)
    }

    fn command_with_data_in(&mut self, op: OpCode, params: &[u32]) -> Result<Vec<u8>> {
        let transaction_id = self.next_transaction_id();
        self.write_container(
            ContainerType::Command,
            op as u16,
            transaction_id,
            params,
            &[],
        )?;

        let data = self.read_container()?;
        if data.container_type != ContainerType::Data || data.transaction_id != transaction_id {
            return Err(AppError::Mtp(format!("Expected data for {op:?}")));
        }

        let response = self.read_container()?;
        ensure_response(response, transaction_id)?;
        Ok(data.payload)
    }

    fn command_with_data_out(&mut self, op: OpCode, params: &[u32], payload: &[u8]) -> Result<()> {
        self.command_with_data_out_response(op, params, payload)
            .map(|_| ())
    }

    fn command_with_data_out_response(
        &mut self,
        op: OpCode,
        params: &[u32],
        payload: &[u8],
    ) -> Result<Vec<u32>> {
        let transaction_id = self.next_transaction_id();
        self.write_container(
            ContainerType::Command,
            op as u16,
            transaction_id,
            params,
            &[],
        )?;
        self.write_container(ContainerType::Data, op as u16, transaction_id, &[], payload)?;
        let response = self.read_container()?;
        ensure_response(response, transaction_id)
    }

    fn next_transaction_id(&mut self) -> u32 {
        let id = self.transaction_id;
        self.transaction_id = self.transaction_id.wrapping_add(1).max(1);
        id
    }

    fn write_container(
        &mut self,
        container_type: ContainerType,
        code: u16,
        transaction_id: u32,
        params: &[u32],
        payload: &[u8],
    ) -> Result<()> {
        let length = 12 + params.len() * 4 + payload.len();
        let mut bytes = Vec::with_capacity(length);
        bytes.extend_from_slice(&(length as u32).to_le_bytes());
        bytes.extend_from_slice(&(container_type as u16).to_le_bytes());
        bytes.extend_from_slice(&code.to_le_bytes());
        bytes.extend_from_slice(&transaction_id.to_le_bytes());

        for param in params {
            bytes.extend_from_slice(&param.to_le_bytes());
        }
        bytes.extend_from_slice(payload);

        self.write_bulk_all(&bytes)?;
        Ok(())
    }

    fn write_bulk_all(&self, mut bytes: &[u8]) -> Result<()> {
        while !bytes.is_empty() {
            let written = self.handle.write_bulk(self.bulk_out, bytes, USB_TIMEOUT)?;
            if written == 0 {
                return Err(AppError::Mtp(
                    "USB device accepted a zero-length write".to_string(),
                ));
            }
            bytes = &bytes[written..];
        }
        Ok(())
    }

    fn read_container(&mut self) -> Result<MtpPacket> {
        let mut buffer = vec![0; 64 * 1024];
        let size = self
            .handle
            .read_bulk(self.bulk_in, &mut buffer, USB_TIMEOUT)?;
        buffer.truncate(size);

        if buffer.len() < 12 {
            return Err(AppError::Mtp("Short MTP packet".to_string()));
        }

        let length = read_u32(&buffer, 0)? as usize;
        while buffer.len() < length {
            let mut chunk = vec![0; 64 * 1024];
            let size = self
                .handle
                .read_bulk(self.bulk_in, &mut chunk, USB_TIMEOUT)?;
            if size == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..size]);
        }

        if buffer.len() < length {
            return Err(AppError::Mtp("Incomplete MTP packet".to_string()));
        }

        let container_type = match read_u16(&buffer, 4)? {
            1 => ContainerType::Command,
            2 => ContainerType::Data,
            3 => ContainerType::Response,
            4 => ContainerType::Event,
            other => {
                return Err(AppError::Mtp(format!("Unknown MTP container type {other}")));
            }
        };
        let code = read_u16(&buffer, 6)?;
        let transaction_id = read_u32(&buffer, 8)?;

        let payload_end = length.min(buffer.len());
        Ok(MtpPacket {
            container_type,
            code,
            transaction_id,
            payload: buffer[12..payload_end].to_vec(),
        })
    }
}

impl Drop for MtpSession {
    fn drop(&mut self) {
        let _ = self.command(OpCode::CloseSession, &[]);
        let _ = self.handle.release_interface(self.interface_number);
    }
}

struct MtpPacket {
    container_type: ContainerType,
    code: u16,
    transaction_id: u32,
    payload: Vec<u8>,
}

fn ensure_response(packet: MtpPacket, transaction_id: u32) -> Result<Vec<u32>> {
    if packet.container_type != ContainerType::Response || packet.transaction_id != transaction_id {
        return Err(AppError::Mtp("Unexpected MTP response packet".to_string()));
    }

    if packet.code != ResponseCode::OK as u16 {
        let code = ResponseCode::from_u16(packet.code)
            .map(|code| format!("{code:?}"))
            .unwrap_or_else(|| format!("0x{:04x}", packet.code));
        return Err(AppError::Mtp(format!("MTP command failed: {code}")));
    }

    packet
        .payload
        .chunks_exact(4)
        .map(|chunk| Ok(u32::from_le_bytes(chunk.try_into().unwrap())))
        .collect()
}

fn parse_u32_array(data: &[u8]) -> Result<Vec<u32>> {
    if data.len() < 4 {
        return Err(AppError::Mtp("Malformed MTP u32 array".to_string()));
    }

    let count = read_u32(data, 0)? as usize;
    let mut values = Vec::with_capacity(count);
    for index in 0..count {
        values.push(read_u32(data, 4 + index * 4)?);
    }
    Ok(values)
}

fn parse_storage_description(data: &[u8]) -> Option<String> {
    let volume_label_offset = skip_mtp_string(data, 26)?;
    parse_mtp_string(data, volume_label_offset)
        .filter(|label| !label.is_empty())
        .or_else(|| parse_mtp_string(data, 26))
}

fn parse_object_info(handle: u32, data: &[u8]) -> Result<ObjectInfo> {
    if data.len() < 53 {
        return Err(AppError::Mtp("Malformed object info dataset".to_string()));
    }

    let storage_id = read_u32(data, 0)?;
    let format = read_u16(data, 4)?;
    let compressed_size = read_u32(data, 8)? as u64;
    let _parent = read_u32(data, 38)?;
    let name = parse_mtp_string(data, 52).unwrap_or_else(|| format!("Object {handle}"));
    let capture_date_offset = skip_mtp_string(data, 52).unwrap_or(53);
    let modified_date_offset =
        skip_mtp_string(data, capture_date_offset).unwrap_or(capture_date_offset);
    let modified =
        parse_mtp_string(data, modified_date_offset).and_then(|value| parse_mtp_date(&value));

    Ok(ObjectInfo {
        handle,
        storage_id,
        name,
        size: compressed_size,
        format,
        modified,
    })
}

fn parse_mtp_string(data: &[u8], offset: usize) -> Option<String> {
    let char_count = *data.get(offset)? as usize;
    if char_count == 0 {
        return Some(String::new());
    }

    let byte_start = offset + 1;
    let byte_len = char_count.saturating_sub(1) * 2;
    let byte_end = byte_start.checked_add(byte_len)?;
    let bytes = data.get(byte_start..byte_end)?;
    let chars = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();

    String::from_utf16(&chars).ok()
}

fn skip_mtp_string(data: &[u8], offset: usize) -> Option<usize> {
    let char_count = *data.get(offset)? as usize;
    Some(offset + 1 + char_count * 2)
}

fn encode_mtp_string(value: &str) -> Vec<u8> {
    let mut chars: Vec<u16> = value.encode_utf16().collect();
    chars.push(0);
    let mut bytes = Vec::with_capacity(1 + chars.len() * 2);
    bytes.push(chars.len().min(u8::MAX as usize) as u8);
    for ch in chars.into_iter().take(u8::MAX as usize) {
        bytes.extend_from_slice(&ch.to_le_bytes());
    }
    bytes
}

fn encode_object_info(
    storage_id: u32,
    parent: u32,
    format: u16,
    association_type: u16,
    size: u32,
    filename: &str,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&storage_id.to_le_bytes());
    bytes.extend_from_slice(&format.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&size.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&parent.to_le_bytes());
    bytes.extend_from_slice(&association_type.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&encode_mtp_string(filename));
    bytes.extend_from_slice(&encode_mtp_string(""));
    bytes.extend_from_slice(&encode_mtp_string(""));
    bytes.extend_from_slice(&encode_mtp_string(""));
    bytes
}

fn mtp_size_field(size: u64) -> u32 {
    u32::try_from(size).unwrap_or(LARGE_OBJECT_MARKER)
}

fn mtp_container_length_field(length: u64) -> u32 {
    u32::try_from(length).unwrap_or(LARGE_OBJECT_MARKER)
}

fn parse_mtp_date(value: &str) -> Option<DateTime<Utc>> {
    let trimmed = value.trim_end_matches('Z');
    NaiveDateTime::parse_from_str(trimmed, "%Y%m%dT%H%M%S")
        .ok()
        .map(|datetime| DateTime::<Utc>::from_naive_utc_and_offset(datetime, Utc))
}

fn read_u16(data: &[u8], offset: usize) -> Result<u16> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or_else(|| AppError::Mtp("Unexpected end of MTP packet".to_string()))?;
    Ok(u16::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| AppError::Mtp("Unexpected end of MTP packet".to_string()))?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

pub fn normalize_remote_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return "/".to_string();
    }

    format!("/{}", trimmed.trim_matches('/'))
}

pub fn join_remote_path(parent: &str, child: &str) -> String {
    let parent = normalize_remote_path(parent);
    if parent == "/" {
        format!("/{}", child.trim_matches('/'))
    } else {
        format!("{}/{}", parent, child.trim_matches('/'))
    }
}

#[cfg(test)]
mod tests {
    use super::{LARGE_OBJECT_MARKER, mtp_container_length_field, mtp_size_field};

    #[test]
    fn encodes_large_object_sizes_with_the_mtp_marker() {
        assert_eq!(mtp_size_field(u32::MAX as u64), u32::MAX);
        assert_eq!(mtp_size_field(u32::MAX as u64 + 1), LARGE_OBJECT_MARKER);
        assert_eq!(
            mtp_container_length_field(u32::MAX as u64 + 1),
            LARGE_OBJECT_MARKER
        );
    }
}

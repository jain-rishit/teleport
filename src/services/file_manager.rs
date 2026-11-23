use crate::domain::error::{AppError, Result};
use crate::domain::models::{FileEntry, FileType};
use crate::infrastructure::mtp::session::{MtpSession, join_remote_path};
use chrono::{DateTime, Utc};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct FileManager;

impl Default for FileManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FileManager {
    pub fn new() -> Self {
        Self
    }

    pub async fn list_local_files(
        &self,
        path: &Path,
        show_hidden_files: bool,
    ) -> Result<Vec<FileEntry>> {
        let path = path.to_path_buf();

        tokio::task::spawn_blocking(move || list_local_files_sync(&path, show_hidden_files))
            .await
            .map_err(|e| AppError::Internal(format!("Local file task failed: {e}")))?
    }

    pub async fn list_files(
        &self,
        device_id: &str,
        path: &str,
        show_hidden_files: bool,
    ) -> Result<Vec<FileEntry>> {
        let device_id = device_id.to_string();
        let path = path.to_string();

        tokio::task::spawn_blocking(move || {
            let mut session = MtpSession::open(&device_id)?;
            if path == "/" {
                return session.list_storages().map(|storages| {
                    storages
                        .into_iter()
                        .map(|storage| FileEntry {
                            name: storage.description.clone(),
                            path: join_remote_path("/", &storage.description),
                            size: 0,
                            file_type: FileType::Directory,
                            modified: None,
                            is_remote: true,
                            storage_id: Some(storage.id),
                            object_id: None,
                        })
                        .collect()
                });
            }

            let Some((storage_id, object_id, resolved_path)) = session.resolve_path(&path)? else {
                return Ok(Vec::new());
            };
            let parent = object_id.unwrap_or(0xFFFF_FFFF);
            let mut files = session.list_files(storage_id, parent, &resolved_path)?;
            if !show_hidden_files {
                files.retain(|file| !is_hidden_name(&file.name));
            }
            Ok(files)
        })
        .await
        .map_err(|e| AppError::Internal(format!("MTP task failed: {e}")))?
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn copy_remote_to_local(
        &self,
        device_id: &str,
        remote_path: &str,
        file_name: &str,
        destination_dir: &Path,
        is_dir: bool,
        delete_source: bool,
        progress: Option<
            std::sync::Arc<std::sync::Mutex<Option<crate::domain::models::TransferProgress>>>,
        >,
    ) -> Result<PathBuf> {
        let device_id = device_id.to_string();
        let remote_path = remote_path.to_string();
        let file_name = file_name.to_string();
        let destination_dir = destination_dir.to_path_buf();

        tokio::task::spawn_blocking(move || {
            if !destination_dir.is_dir() {
                return Err(AppError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Destination folder does not exist",
                )));
            }

            let mut session = MtpSession::open(&device_id)?;
            let destination = if is_dir {
                let Some((storage_id, object_id, resolved_path)) =
                    session.resolve_path(&remote_path)?
                else {
                    return Err(AppError::Mtp(format!(
                        "Remote folder '{remote_path}' not found"
                    )));
                };
                let parent = object_id.unwrap_or(0xFFFF_FFFF);
                let destination = unique_destination_dir(&destination_dir, &file_name);
                fs::create_dir_all(&destination)?;
                download_remote_folder(
                    &mut session,
                    storage_id,
                    parent,
                    &resolved_path,
                    &destination,
                    progress,
                )?;
                destination
            } else {
                download_remote_file(
                    &mut session,
                    &remote_path,
                    &file_name,
                    &destination_dir,
                    progress,
                )?
            };

            if delete_source {
                delete_remote_entry(&mut session, &remote_path)?;
            }

            Ok(destination)
        })
        .await
        .map_err(|e| AppError::Internal(format!("MTP task failed: {e}")))?
    }

    pub async fn copy_local_to_remote(
        &self,
        device_id: &str,
        local_path: &Path,
        remote_parent_path: &str,
        delete_source: bool,
        progress: Option<
            std::sync::Arc<std::sync::Mutex<Option<crate::domain::models::TransferProgress>>>,
        >,
    ) -> Result<()> {
        let device_id = device_id.to_string();
        let local_path = local_path.to_path_buf();
        let remote_parent_path = remote_parent_path.to_string();

        tokio::task::spawn_blocking(move || {
            let mut session = MtpSession::open(&device_id)?;
            upload_local_entry(&mut session, &local_path, &remote_parent_path, progress)?;

            if delete_source {
                let metadata = fs::metadata(&local_path)?;
                if metadata.is_dir() {
                    fs::remove_dir_all(&local_path)?;
                } else {
                    fs::remove_file(&local_path)?;
                }
            }

            Ok(())
        })
        .await
        .map_err(|e| AppError::Internal(format!("MTP task failed: {e}")))?
    }

    pub async fn delete_file(&self, device_id: &str, remote_path: &str) -> Result<()> {
        let device_id = device_id.to_string();
        let remote_path = remote_path.to_string();

        tokio::task::spawn_blocking(move || {
            let mut session = MtpSession::open(&device_id)?;
            let Some((_, Some(object_id), _)) = session.resolve_path(&remote_path)? else {
                return Err(AppError::Mtp(format!(
                    "Remote path '{remote_path}' not found"
                )));
            };

            session.delete_object(object_id)
        })
        .await
        .map_err(|e| AppError::Internal(format!("MTP task failed: {e}")))?
    }

    pub async fn delete_local(&self, path: &Path) -> Result<()> {
        let path = path.to_path_buf();

        tokio::task::spawn_blocking(move || {
            let metadata = fs::metadata(&path)?;
            if metadata.is_dir() {
                fs::remove_dir_all(&path)?;
            } else {
                fs::remove_file(&path)?;
            }
            Ok(())
        })
        .await
        .map_err(|e| AppError::Internal(format!("Local file task failed: {e}")))?
    }

    pub async fn open_local_file(&self, path: &Path, quick_look: bool) -> Result<()> {
        let path = path.to_path_buf();

        tokio::task::spawn_blocking(move || open_path(&path, quick_look))
            .await
            .map_err(|e| AppError::Internal(format!("Local file task failed: {e}")))?
    }

    pub async fn open_remote_file(
        &self,
        device_id: &str,
        remote_path: &str,
        file_name: &str,
        is_dir: bool,
        quick_look: bool,
    ) -> Result<PathBuf> {
        let device_id = device_id.to_string();
        let remote_path = remote_path.to_string();
        let file_name = file_name.to_string();

        tokio::task::spawn_blocking(move || {
            let preview_dir = std::env::temp_dir().join("teleport-preview");
            fs::create_dir_all(&preview_dir)?;

            let mut session = MtpSession::open(&device_id)?;
            let preview_path = if is_dir {
                let Some((storage_id, object_id, resolved_path)) =
                    session.resolve_path(&remote_path)?
                else {
                    return Err(AppError::Mtp(format!(
                        "Remote folder '{remote_path}' not found"
                    )));
                };
                let parent = object_id.unwrap_or(0xFFFF_FFFF);
                let preview_path = unique_destination_dir(&preview_dir, &file_name);
                fs::create_dir_all(&preview_path)?;
                download_remote_folder(
                    &mut session,
                    storage_id,
                    parent,
                    &resolved_path,
                    &preview_path,
                    None,
                )?;
                preview_path
            } else {
                download_remote_file(&mut session, &remote_path, &file_name, &preview_dir, None)?
            };
            open_path(&preview_path, quick_look)?;
            // Clean up temp preview files after launching Quick Look
            if quick_look {
                if is_dir {
                    let _ = fs::remove_dir_all(&preview_path);
                } else {
                    let _ = fs::remove_file(&preview_path);
                }
            }
            Ok(preview_path)
        })
        .await
        .map_err(|e| AppError::Internal(format!("MTP task failed: {e}")))?
    }

    pub async fn choose_local_folder(&self) -> Result<Option<PathBuf>> {
        tokio::task::spawn_blocking(move || {
            let output = Command::new("osascript")
                .arg("-e")
                .arg(r#"POSIX path of (choose folder with prompt "Choose a Teleport destination folder")"#)
                .output()?;

            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if path.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(PathBuf::from(path)))
                }
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if stderr.contains("User canceled") {
                    Ok(None)
                } else {
                    Err(AppError::Internal(format!(
                        "Folder chooser failed: {}",
                        stderr.trim()
                    )))
                }
            }
        })
        .await
        .map_err(|e| AppError::Internal(format!("Folder chooser task failed: {e}")))?
    }

    pub async fn rename_file(
        &self,
        device_id: &str,
        remote_path: &str,
        new_name: &str,
    ) -> Result<()> {
        let device_id = device_id.to_string();
        let remote_path = remote_path.to_string();
        let new_name = new_name.to_string();

        tokio::task::spawn_blocking(move || {
            if new_name.trim().is_empty() || new_name.contains('/') {
                return Err(AppError::Mtp(
                    "New name must be a single non-empty filename".to_string(),
                ));
            }

            let mut session = MtpSession::open(&device_id)?;
            let Some((_, Some(object_id), _)) = session.resolve_path(&remote_path)? else {
                return Err(AppError::Mtp(format!(
                    "Remote path '{remote_path}' not found"
                )));
            };

            session.rename_object(object_id, &new_name)
        })
        .await
        .map_err(|e| AppError::Internal(format!("MTP task failed: {e}")))?
    }

    pub async fn rename_local(&self, path: &Path, new_name: &str) -> Result<PathBuf> {
        let path = path.to_path_buf();
        let new_name = validate_single_name(new_name)?;

        tokio::task::spawn_blocking(move || {
            let parent = path.parent().ok_or_else(|| {
                AppError::Internal("Cannot rename a path without a parent".to_string())
            })?;
            let destination = parent.join(new_name);
            fs::rename(&path, &destination)?;
            Ok(destination)
        })
        .await
        .map_err(|e| AppError::Internal(format!("Local file task failed: {e}")))?
    }

    pub async fn create_local_folder(&self, parent: &Path, name: &str) -> Result<PathBuf> {
        let parent = parent.to_path_buf();
        let name = validate_single_name(name)?;

        tokio::task::spawn_blocking(move || {
            let destination = parent.join(name);
            fs::create_dir(&destination)?;
            Ok(destination)
        })
        .await
        .map_err(|e| AppError::Internal(format!("Local file task failed: {e}")))?
    }

    pub async fn create_remote_folder(
        &self,
        device_id: &str,
        remote_parent_path: &str,
        name: &str,
    ) -> Result<()> {
        let device_id = device_id.to_string();
        let remote_parent_path = remote_parent_path.to_string();
        let name = validate_single_name(name)?;

        tokio::task::spawn_blocking(move || {
            let mut session = MtpSession::open(&device_id)?;
            session.create_folder(&remote_parent_path, &name)
        })
        .await
        .map_err(|e| AppError::Internal(format!("MTP task failed: {e}")))?
    }
}

fn list_local_files_sync(path: &Path, show_hidden_files: bool) -> Result<Vec<FileEntry>> {
    let mut files = Vec::new();

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let entry_path = entry.path();
        let name = entry
            .file_name()
            .to_str()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| entry_path.display().to_string());

        if !show_hidden_files && is_hidden_name(&name) {
            continue;
        }

        let modified = metadata.modified().ok().map(DateTime::<Utc>::from);

        files.push(FileEntry {
            name,
            path: entry_path.to_string_lossy().to_string(),
            size: if metadata.is_dir() { 0 } else { metadata.len() },
            file_type: if metadata.is_dir() {
                FileType::Directory
            } else {
                FileType::File
            },
            modified,
            is_remote: false,
            storage_id: None,
            object_id: None,
        });
    }

    files.sort_by(|a, b| {
        b.is_dir()
            .cmp(&a.is_dir())
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(files)
}

fn is_hidden_name(name: &str) -> bool {
    name.starts_with('.')
}

fn download_remote_file(
    session: &mut MtpSession,
    remote_path: &str,
    file_name: &str,
    destination_dir: &Path,
    progress: Option<
        std::sync::Arc<std::sync::Mutex<Option<crate::domain::models::TransferProgress>>>,
    >,
) -> Result<PathBuf> {
    let (_, object_id, _) = resolve_existing_object(session, remote_path)?;
    let destination = unique_destination(destination_dir, file_name);

    let file_name_str = file_name.to_string();
    let started_at = std::time::Instant::now();
    let on_progress = |bytes_transferred, total_bytes| {
        if let Some(ref p) = progress
            && let Ok(mut guard) = p.lock()
        {
            *guard = Some(crate::domain::models::TransferProgress {
                file_name: file_name_str.clone(),
                bytes_transferred,
                total_bytes,
                started_at,
            });
        }
    };

    session.download_object_to_file(object_id, &destination, on_progress)?;
    Ok(destination)
}

fn download_remote_folder(
    session: &mut MtpSession,
    storage_id: u32,
    parent: u32,
    remote_path: &str,
    destination: &Path,
    progress: Option<
        std::sync::Arc<std::sync::Mutex<Option<crate::domain::models::TransferProgress>>>,
    >,
) -> Result<()> {
    for child in session.list_files(storage_id, parent, remote_path)? {
        if child.is_dir() {
            let child_parent = child.object_id.ok_or_else(|| {
                AppError::Mtp(format!("Remote folder '{}' has no object id", child.path))
            })?;
            let child_destination = unique_destination_dir(destination, &child.name);
            fs::create_dir_all(&child_destination)?;
            download_remote_folder(
                session,
                storage_id,
                child_parent,
                &child.path,
                &child_destination,
                progress.clone(),
            )?;
        } else {
            let object_id = child.object_id.ok_or_else(|| {
                AppError::Mtp(format!("Remote file '{}' has no object id", child.path))
            })?;
            let dest_file = unique_destination(destination, &child.name);

            let file_name_str = child.name.clone();
            let started_at = std::time::Instant::now();
            let progress_clone = progress.clone();
            let on_progress = move |bytes_transferred, total_bytes| {
                if let Some(ref p) = progress_clone
                    && let Ok(mut guard) = p.lock()
                {
                    *guard = Some(crate::domain::models::TransferProgress {
                        file_name: file_name_str.clone(),
                        bytes_transferred,
                        total_bytes,
                        started_at,
                    });
                }
            };

            session.download_object_to_file(object_id, &dest_file, on_progress)?;
        }
    }

    Ok(())
}

fn upload_local_entry(
    session: &mut MtpSession,
    local_path: &Path,
    remote_parent_path: &str,
    progress: Option<
        std::sync::Arc<std::sync::Mutex<Option<crate::domain::models::TransferProgress>>>,
    >,
) -> Result<()> {
    let name = file_name_string(local_path)?;
    let metadata = fs::metadata(local_path)?;

    if metadata.is_dir() {
        session.create_folder(remote_parent_path, &name)?;
        let child_remote_parent = join_remote_path(remote_parent_path, &name);

        for entry in fs::read_dir(local_path)? {
            let entry = entry?;
            upload_local_entry(
                session,
                &entry.path(),
                &child_remote_parent,
                progress.clone(),
            )?;
        }
    } else {
        let file_name_str = name.clone();
        let started_at = std::time::Instant::now();
        let on_progress = |bytes_transferred, total_bytes| {
            if let Some(ref p) = progress
                && let Ok(mut guard) = p.lock()
            {
                *guard = Some(crate::domain::models::TransferProgress {
                    file_name: file_name_str.clone(),
                    bytes_transferred,
                    total_bytes,
                    started_at,
                });
            }
        };

        session.upload_object_streaming(remote_parent_path, &name, local_path, on_progress)?;
    }

    Ok(())
}

fn resolve_existing_object(
    session: &mut MtpSession,
    remote_path: &str,
) -> Result<(u32, u32, String)> {
    let Some((storage_id, Some(object_id), resolved_path)) = session.resolve_path(remote_path)?
    else {
        return Err(AppError::Mtp(format!(
            "Remote path '{remote_path}' not found"
        )));
    };

    Ok((storage_id, object_id, resolved_path))
}

fn delete_remote_entry(session: &mut MtpSession, remote_path: &str) -> Result<()> {
    let (_, object_id, _) = resolve_existing_object(session, remote_path)?;
    session.delete_object(object_id)
}

fn open_path(path: &Path, quick_look: bool) -> Result<()> {
    let mut command = if quick_look {
        let mut command = Command::new("qlmanage");
        command.arg("-p");
        command
    } else {
        Command::new("open")
    };

    command.arg(path).spawn()?;
    Ok(())
}

fn file_name_string(path: &Path) -> Result<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
        .ok_or_else(|| AppError::Internal("Invalid filename".to_string()))
}

fn validate_single_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() || name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        return Err(AppError::Internal(
            "Name must be a single non-empty filename".to_string(),
        ));
    }

    Ok(name.to_string())
}

fn unique_destination(dir: &Path, file_name: &str) -> PathBuf {
    let candidate = dir.join(file_name);
    if !candidate.exists() {
        return candidate;
    }

    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("download");
    let extension = Path::new(file_name)
        .extension()
        .and_then(|ext| ext.to_str());

    for index in 1.. {
        let name = match extension {
            Some(ext) => format!("{stem} ({index}).{ext}"),
            None => format!("{stem} ({index})"),
        };
        let candidate = dir.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }

    unreachable!()
}

fn unique_destination_dir(dir: &Path, folder_name: &str) -> PathBuf {
    let candidate = dir.join(folder_name);
    if !candidate.exists() {
        return candidate;
    }

    for index in 1.. {
        let candidate = dir.join(format!("{folder_name} ({index})"));
        if !candidate.exists() {
            return candidate;
        }
    }

    unreachable!()
}

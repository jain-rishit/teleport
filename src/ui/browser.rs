use std::path::PathBuf;

use crate::models;
use crate::ui::{AppView, PendingAction, ViewName};
use gpui::prelude::*;
use gpui::{Context, Div, IntoElement, MouseButton, Timer, div, px, rgb, rgba};

fn t(s: impl IntoElement) -> Div {
    div().child(s)
}

fn dark_colors(
    dark: bool,
) -> (
    gpui::Rgba,
    gpui::Rgba,
    gpui::Rgba,
    gpui::Rgba,
    gpui::Rgba,
    gpui::Rgba,
) {
    if dark {
        (
            rgb(0xf5f5f7),    // text
            rgb(0xa0a0a5),    // secondary
            rgb(0x2c2c2e),    // surface
            rgb(0x2c2c2e),    // row
            rgba(0xffffff14), // border
            rgb(0x636366),    // muted
        )
    } else {
        (
            rgb(0x1d1d1f),
            rgb(0x6e6e73),
            rgb(0xffffff),
            rgb(0xffffff),
            rgba(0x0000001a),
            rgb(0xaeaeb2),
        )
    }
}

fn file_type_color(ext: &str) -> gpui::Rgba {
    match ext {
        "jpg" | "jpeg" | "png" | "heic" | "webp" | "gif" => rgb(0x5aa7ff),
        "mp4" | "mov" | "mkv" | "avi" | "webm" => rgb(0xf8c14a),
        "mp3" | "wav" | "m4a" | "flac" | "ogg" => rgb(0x71e0a5),
        "pdf" => rgb(0xff7b72),
        "zip" | "rar" | "7z" | "tar" | "gz" => rgb(0xb7c0ce),
        "txt" | "md" | "json" | "csv" | "doc" | "docx" => rgb(0xb7c0ce),
        "apk" => rgb(0xb7c0ce),
        _ => rgb(0xb7c0ce),
    }
}

fn file_icon_char(name: &str, is_dir: bool) -> &'static str {
    if is_dir {
        return "F";
    }
    let ext = std::path::Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "jpg" | "jpeg" | "png" | "heic" | "webp" | "gif" => "I",
        "mp4" | "mov" | "mkv" | "avi" | "webm" => "V",
        "mp3" | "wav" | "m4a" | "flac" | "ogg" => "A",
        "pdf" => "P",
        "zip" | "rar" | "7z" | "tar" | "gz" => "Z",
        "txt" | "md" | "json" | "csv" | "doc" | "docx" => "D",
        "apk" => "D",
        _ => "f",
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    match bytes {
        size if size >= GB => format!("{:.1} GB", size as f64 / GB as f64),
        size if size >= MB => format!("{:.1} MB", size as f64 / MB as f64),
        size if size >= KB => format!("{:.1} KB", size as f64 / KB as f64),
        size => format!("{size} B"),
    }
}

fn spawn_transfer_progress_poller(
    cx: &mut Context<AppView>,
    progress: std::sync::Arc<std::sync::Mutex<Option<models::TransferProgress>>>,
) {
    cx.spawn(async move |view: gpui::WeakEntity<AppView>, cx| {
        loop {
            Timer::after(std::time::Duration::from_millis(100)).await;
            let snapshot = progress.lock().ok().and_then(|guard| guard.clone());
            let keep_polling = view
                .update(cx, move |this, cx| {
                    if !this.show_transfer_popup {
                        return false;
                    }

                    if let Some(progress) = snapshot {
                        this.transfer_file_name = progress.file_name;
                        this.transfer_bytes = progress.bytes_transferred;
                        this.transfer_total = progress.total_bytes;
                        this.transfer_percent = if progress.total_bytes == 0 {
                            0.0
                        } else {
                            (progress.bytes_transferred as f64 / progress.total_bytes as f64
                                * 100.0) as f32
                        };
                        let elapsed = progress.started_at.elapsed().as_secs_f64();
                        this.transfer_speed = if elapsed > 0.0 {
                            format!(
                                "{}/s",
                                format_bytes((progress.bytes_transferred as f64 / elapsed) as u64)
                            )
                        } else {
                            String::new()
                        };
                        cx.notify();
                    }
                    true
                })
                .unwrap_or(false);

            if !keep_polling {
                break;
            }
        }
    })
    .detach();
}

pub(crate) fn render_file_row(
    app: &AppView,
    entry: &models::FileEntry,
    _index: usize,
    _pane: &str,
    cx: &mut Context<AppView>,
) -> Div {
    let is_dir = entry.is_dir();
    let is_remote = entry.is_remote;
    let (text, _secondary, _surface, row, border, muted) = dark_colors(app.is_dark_theme);
    let ext = std::path::Path::new(&entry.name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    let icon_color = if is_dir {
        rgb(0x5aa7ff)
    } else {
        file_type_color(&ext)
    };

    let name = entry.name.clone();
    let path = entry.path.clone();
    let size = entry.size;
    let device_id = app.connected_device.as_ref().map(|d| d.id.clone());
    let default_dest = app.default_destination.clone();
    let local_path = app.local_path.clone();
    let remote_path = app.remote_path.clone();
    let show_hidden = app.show_hidden_files;

    let modified = entry
        .modified
        .map(|m| m.format("%d.%m.%Y").to_string())
        .unwrap_or_default();
    let size_label = if is_dir {
        "Folder".to_string()
    } else {
        format_bytes(size)
    };
    let detail = format!("{} • {}", size_label, modified);

    // Copy action (remote -> local download, or local -> remote upload)
    let copy_action = {
        let device_id = device_id.clone();
        let path = path.clone();
        let name = name.clone();
        let default_dest = default_dest.clone();
        let local_path = local_path.clone();
        let remote_path = remote_path.clone();
        cx.listener(move |this, _, _, cx| {
            if is_remote {
                let device_id = device_id.clone().unwrap_or_default();
                let remote_path = path.clone();
                let file_name = name.clone();
                let dest = default_dest.clone();
                let local_path_after = local_path.clone();

                this.show_transfer_popup = true;
                this.transfer_file_name = file_name.clone();
                this.transfer_complete = false;
                this.transfer_bytes = 0;
                this.transfer_total = size;
                this.transfer_percent = 0.0;
                this.transfer_speed.clear();
                cx.notify();

                let file_manager = this.file_manager.clone();
                let progress = std::sync::Arc::new(std::sync::Mutex::new(None));
                spawn_transfer_progress_poller(cx, progress.clone());
                cx.spawn(async move |view: gpui::WeakEntity<AppView>, cx| {
                    let result = file_manager
                        .copy_remote_to_local(
                            &device_id,
                            &remote_path,
                            &file_name,
                            &dest,
                            is_dir,
                            false,
                            Some(progress),
                        )
                        .await;
                    let refreshed = if result.is_ok() {
                        file_manager
                            .list_local_files(&local_path_after, show_hidden)
                            .await
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };

                    let _ = view.update(cx, move |this, cx| match result {
                        Ok(_) => {
                            this.transfer_complete = true;
                            this.transfer_bytes = this.transfer_total;
                            this.transfer_percent = 100.0;
                            this.local_files = refreshed;
                            this.show_transfer_popup = false;
                            this.set_status("Download complete", false);
                            cx.notify();
                        }
                        Err(e) => {
                            this.transfer_complete = true;
                            this.show_transfer_popup = false;
                            this.set_status(format!("Download failed: {e}"), true);
                            cx.notify();
                        }
                    });
                })
                .detach();
            } else {
                let device_id = device_id.clone().unwrap_or_default();
                let local_path_buf = PathBuf::from(path.clone());
                let file_name = name.clone();
                let remote_dir = remote_path.clone();

                this.show_transfer_popup = true;
                this.transfer_file_name = file_name.clone();
                this.transfer_complete = false;
                this.transfer_bytes = 0;
                this.transfer_total = size;
                this.transfer_percent = 0.0;
                this.transfer_speed.clear();
                cx.notify();

                let file_manager = this.file_manager.clone();
                let progress = std::sync::Arc::new(std::sync::Mutex::new(None));
                spawn_transfer_progress_poller(cx, progress.clone());
                cx.spawn(async move |view: gpui::WeakEntity<AppView>, cx| {
                    let result = file_manager
                        .copy_local_to_remote(
                            &device_id,
                            &local_path_buf,
                            &remote_dir,
                            false,
                            Some(progress),
                        )
                        .await;
                    let refreshed = if result.is_ok() {
                        file_manager
                            .list_files(&device_id, &remote_dir, show_hidden)
                            .await
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };

                    let _ = view.update(cx, move |this, cx| match result {
                        Ok(_) => {
                            this.transfer_complete = true;
                            this.transfer_bytes = this.transfer_total;
                            this.transfer_percent = 100.0;
                            this.remote_files = refreshed;
                            this.show_transfer_popup = false;
                            this.set_status("Upload complete", false);
                            cx.notify();
                        }
                        Err(e) => {
                            this.transfer_complete = true;
                            this.show_transfer_popup = false;
                            this.set_status(format!("Upload failed: {e}"), true);
                            cx.notify();
                        }
                    });
                })
                .detach();
            }
        })
    };

    // Delete action
    let delete_action = {
        let name = name.clone();
        let path = path.clone();
        let device_id = device_id.clone();
        cx.listener(move |this, _, _, cx| {
            this.show_confirm = true;
            this.confirm_title = "Delete".to_string();
            this.confirm_message = format!("Delete \"{}\"?", name);
            if is_remote {
                this.pending_action = Some(PendingAction::DeleteRemote {
                    device_id: device_id.clone().unwrap_or_default(),
                    remote_path: path.clone(),
                });
            } else {
                this.pending_action = Some(PendingAction::DeleteLocal {
                    path: PathBuf::from(path.clone()),
                });
            }
            cx.notify();
        })
    };

    div()
        .flex()
        .items_center()
        .justify_between()
        .px(px(12.0))
        .py(px(9.0))
        .bg(row)
        .border_b(px(1.0))
        .border_color(border)
        .child(
            div()
                .flex()
                .items_center()
                .gap_10()
                .child(
                    div()
                        .w(px(32.0))
                        .h(px(32.0))
                        .bg(icon_color)
                        .rounded(px(6.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            t(file_icon_char(&name, is_dir))
                                .text_color(rgb(0xffffff))
                                .text_xs(),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(t(name.clone()).text_color(text).text_sm())
                        .child(t(detail.clone()).text_color(muted).text_xs()),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(rgb(0x007aff))
                        .px(px(2.0))
                        .py(px(1.0))
                        .rounded(px(4.0))
                        .child(
                            t(if is_remote { "← Local" } else { "→ Device" })
                                .text_color(rgb(0xffffff))
                                .text_xs(),
                        )
                        .on_mouse_up(MouseButton::Left, copy_action),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(if app.is_dark_theme {
                            rgb(0x48484a)
                        } else {
                            rgb(0xe5e5ea)
                        })
                        .px(px(2.0))
                        .py(px(1.0))
                        .rounded(px(4.0))
                        .child(
                            t("Delete")
                                .text_color(if app.is_dark_theme {
                                    rgb(0xffffff)
                                } else {
                                    rgb(0x1d1d1f)
                                })
                                .text_xs(),
                        )
                        .on_mouse_up(MouseButton::Left, delete_action),
                ),
        )
}

pub(crate) fn render_browser(app: &mut AppView, cx: &mut Context<AppView>) -> Div {
    let primary = rgb(0x007aff);
    let dark = app.is_dark_theme;
    let (text, secondary, surface, _row, border, muted) = dark_colors(dark);

    let device_id = app.connected_device.as_ref().map(|d| d.id.clone());
    let local_path = app.local_path.clone();
    let remote_path = app.remote_path.clone();
    let show_hidden = app.show_hidden_files;

    // Owned display strings, computed before the listeners move the originals.
    let local_path_display = local_path.to_string_lossy().to_string();
    let remote_path_display = remote_path.clone();

    // Top bar
    let top = div()
        .flex()
        .justify_between()
        .items_center()
        .px(px(16.0))
        .py(px(12.0))
        .border_b(px(1.0))
        .border_color(border)
        .bg(surface)
        .child(
            div()
                .flex()
                .items_center()
                .gap_10()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(if dark { rgb(0x48484a) } else { rgb(0xe5e5ea) })
                        .px(px(2.0))
                        .py(px(1.0))
                        .rounded(px(4.0))
                        .child(t("‹").text_color(text).text_lg())
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.current_view = ViewName::Home;
                                cx.notify();
                            }),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            t(app.browser_device_name.clone())
                                .text_color(text)
                                .text_sm(),
                        )
                        .child(
                            t(if app.is_local_loading || app.is_remote_loading {
                                "Loading…"
                            } else {
                                "Ready"
                            })
                            .text_color(muted)
                            .text_xs(),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(if dark { rgb(0x48484a) } else { rgb(0xe5e5ea) })
                        .px(px(2.0))
                        .py(px(1.0))
                        .rounded(px(4.0))
                        .child(t("Refresh").text_color(text).text_xs())
                        .on_mouse_up(MouseButton::Left, {
                            let device_id = device_id.clone();
                            let local_path = local_path.clone();
                            let remote_path = remote_path.clone();
                            cx.listener(move |this, _, _, cx| {
                                let local_path = local_path.clone();
                                let remote_path = remote_path.clone();

                                this.is_local_loading = true;
                                this.is_remote_loading = true;
                                cx.notify();

                                let local = this
                                    .runtime
                                    .block_on(
                                        this.file_manager
                                            .list_local_files(&local_path, show_hidden),
                                    )
                                    .unwrap_or_default();
                                let remote = match &device_id {
                                    Some(id) => this
                                        .runtime
                                        .block_on(this.file_manager.list_files(
                                            id,
                                            &remote_path,
                                            show_hidden,
                                        ))
                                        .unwrap_or_default(),
                                    None => Vec::new(),
                                };

                                this.local_files = local;
                                this.remote_files = remote;
                                this.is_local_loading = false;
                                this.is_remote_loading = false;
                                cx.notify();
                            })
                        }),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(primary)
                        .px(px(2.0))
                        .py(px(1.0))
                        .rounded(px(4.0))
                        .child(t("New Folder").text_color(rgb(0xffffff)).text_xs())
                        .on_mouse_up(MouseButton::Left, {
                            let device_id = device_id.clone();
                            let remote_path = remote_path.clone();
                            let local_path = local_path.clone();
                            let folder_name = "New Folder";
                            cx.listener(move |this, _, _, cx| {
                                match &device_id {
                                    Some(id) => {
                                        let result = this.runtime.block_on(
                                            this.file_manager.create_remote_folder(
                                                id,
                                                &remote_path,
                                                folder_name,
                                            ),
                                        );
                                        match result {
                                            Ok(()) => {
                                                let refreshed = this
                                                    .runtime
                                                    .block_on(this.file_manager.list_files(
                                                        id,
                                                        &remote_path,
                                                        show_hidden,
                                                    ))
                                                    .unwrap_or_default();
                                                this.remote_files = refreshed;
                                                this.set_status("Folder created", false);
                                            }
                                            Err(e) => {
                                                this.set_status(
                                                    format!("Create folder failed: {e}"),
                                                    true,
                                                );
                                            }
                                        }
                                    }
                                    None => {
                                        let result =
                                            std::fs::create_dir(local_path.join(folder_name));
                                        match result {
                                            Ok(()) => {
                                                let refreshed = this
                                                    .runtime
                                                    .block_on(
                                                        this.file_manager.list_local_files(
                                                            &local_path,
                                                            show_hidden,
                                                        ),
                                                    )
                                                    .unwrap_or_default();
                                                this.local_files = refreshed;
                                                this.set_status("Folder created", false);
                                            }
                                            Err(e) => {
                                                this.set_status(
                                                    format!("Create folder failed: {e}"),
                                                    true,
                                                );
                                            }
                                        }
                                    }
                                }
                                cx.notify();
                            })
                        }),
                ),
        );

    // Path row
    let path_row = div()
        .flex()
        .items_center()
        .gap_2()
        .px(px(16.0))
        .py(px(8.0))
        .bg(if dark { rgb(0x1c1c1e) } else { rgb(0xf5f5f7) })
        .child(t("Local:").text_color(muted).text_xs())
        .child(
            div()
                .child(local_path_display.clone())
                .text_color(text)
                .text_sm(),
        )
        .child(t("Remote:").text_color(muted).text_xs())
        .child(
            div()
                .child(remote_path_display.clone())
                .text_color(text)
                .text_sm(),
        );

    // Panes
    let mut local_list = div().flex().flex_col();
    for (i, entry) in app.local_files.iter().enumerate() {
        local_list = local_list.child(render_file_row(app, entry, i, "local", cx));
    }
    if app.local_files.is_empty() {
        local_list = local_list.child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_3()
                .pt(px(24.0))
                .child(t("Empty").text_color(muted).text_sm()),
        );
    }

    let mut remote_list = div().flex().flex_col();
    for (i, entry) in app.remote_files.iter().enumerate() {
        remote_list = remote_list.child(render_file_row(app, entry, i, "remote", cx));
    }
    if app.remote_files.is_empty() {
        remote_list = remote_list.child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_3()
                .pt(px(24.0))
                .child(t("Empty").text_color(muted).text_sm()),
        );
    }

    let pane = |inner: Div| {
        div()
            .flex()
            .flex_col()
            .child(div().flex().flex_col().flex_1().min_h(px(0.0)).child(inner))
    };

    let local_pane = div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(0.0))
        .bg(surface)
        .border(px(1.0))
        .border_color(border)
        .rounded(px(8.0))
        .child(t("Local").text_color(secondary).text_xs().m(px(10.0)))
        .child(pane(local_list));

    let remote_pane = div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(0.0))
        .bg(surface)
        .border(px(1.0))
        .border_color(border)
        .rounded(px(8.0))
        .child(t("Device").text_color(secondary).text_xs().m(px(10.0)))
        .child(pane(remote_list));

    let panes = div()
        .flex()
        .flex_1()
        .min_h(px(0.0))
        .gap_3()
        .p(px(12.0))
        .child(local_pane)
        .child(remote_pane);

    // Loading overlay
    let loading = if app.is_local_loading || app.is_remote_loading {
        div()
            .absolute()
            .inset(px(0.0))
            .flex()
            .items_center()
            .justify_center()
            .bg(if dark {
                rgba(0x0000008c)
            } else {
                rgba(0x00000040)
            })
            .child(t("Loading…").text_color(text).text_sm())
    } else {
        div()
    };

    // Transfer popup
    let transfer_popup = if app.show_transfer_popup {
        render_transfer_popup(app, cx)
    } else {
        div()
    };

    // Confirm popup
    let confirm_popup = if app.show_confirm {
        render_confirm_popup(app, cx)
    } else {
        div()
    };

    div()
        .relative()
        .flex()
        .flex_col()
        .w_full()
        .h_full()
        .child(top)
        .child(path_row)
        .child(panes)
        .child(loading)
        .child(transfer_popup)
        .child(confirm_popup)
}

fn render_transfer_popup(app: &AppView, cx: &mut Context<AppView>) -> Div {
    let dark = app.is_dark_theme;
    let surface = if dark { rgb(0x2c2c2e) } else { rgb(0xffffff) };
    let text = if dark { rgb(0xf5f5f7) } else { rgb(0x1d1d1f) };
    let border = if dark {
        rgba(0xffffff1a)
    } else {
        rgba(0x0000001a)
    };
    let secondary = if dark { rgb(0xa0a0a5) } else { rgb(0x6e6e73) };

    div()
        .absolute()
        .inset(px(0.0))
        .flex()
        .items_center()
        .justify_center()
        .bg(if dark {
            rgba(0x0000008c)
        } else {
            rgba(0x00000040)
        })
        .child(
            div()
                .flex()
                .flex_col()
                .w(px(360.0))
                .gap_4()
                .p(px(20.0))
                .bg(surface)
                .border(px(1.0))
                .border_color(border)
                .rounded(px(10.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_10()
                        .child(
                            div()
                                .w(px(32.0))
                                .h(px(32.0))
                                .bg(rgb(0x007aff))
                                .rounded(px(9999.0)),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(t("Transferring").text_color(text).text_sm())
                                .child(
                                    t(app.transfer_file_name.clone())
                                        .text_color(secondary)
                                        .text_xs(),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .child(t("Progress").text_color(secondary).text_xs())
                                .child(
                                    t(format!("{:.0}%", app.transfer_percent))
                                        .text_color(text)
                                        .text_xs(),
                                ),
                        )
                        .child(
                            div()
                                .w(px(240.0))
                                .h(px(6.0))
                                .bg(if dark {
                                    rgba(0xffffff1a)
                                } else {
                                    rgba(0x00000012)
                                })
                                .rounded(px(9999.0))
                                .child(
                                    div()
                                        .w(px(app.transfer_percent.clamp(0.0, 100.0) * 2.4))
                                        .h(px(6.0))
                                        .bg(rgb(0x007aff))
                                        .rounded(px(9999.0)),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .child(
                                    t(format!(
                                        "{} / {}",
                                        format_bytes(app.transfer_bytes),
                                        format_bytes(app.transfer_total)
                                    ))
                                    .text_color(secondary)
                                    .text_xs(),
                                )
                                .child(
                                    t(app.transfer_speed.clone())
                                        .text_color(secondary)
                                        .text_xs(),
                                ),
                        ),
                )
                .child(
                    div().flex().justify_end().gap_3().child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(if dark { rgb(0x48484a) } else { rgb(0xe5e5ea) })
                            .px(px(3.0))
                            .py(px(1.5))
                            .rounded(px(6.0))
                            .child(t("Hide").text_color(text).text_sm())
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.show_transfer_popup = false;
                                    cx.notify();
                                }),
                            ),
                    ),
                ),
        )
}

fn render_confirm_popup(app: &AppView, cx: &mut Context<AppView>) -> Div {
    let dark = app.is_dark_theme;
    let surface = if dark { rgb(0x2a2a2a) } else { rgb(0xffffff) };
    let text = if dark { rgb(0xf5f5f7) } else { rgb(0x1d1d1f) };
    let danger = rgb(0xff453a);

    div()
        .absolute()
        .inset(px(0.0))
        .flex()
        .items_center()
        .justify_center()
        .bg(if dark {
            rgba(0x0000008c)
        } else {
            rgba(0x00000040)
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap_4()
                .p(px(24.0))
                .bg(surface)
                .rounded(px(12.0))
                .child(t(app.confirm_title.clone()).text_color(text).text_lg())
                .child(
                    t(app.confirm_message.clone())
                        .text_color(rgb(0x636366))
                        .text_sm(),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_3()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(rgb(0xf0f0f5))
                                .px(px(3.0))
                                .py(px(1.5))
                                .rounded(px(6.0))
                                .child(t("Cancel").text_color(rgb(0x1d1d1f)).text_sm())
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        this.show_confirm = false;
                                        this.pending_action = None;
                                        cx.notify();
                                    }),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(danger)
                                .px(px(3.0))
                                .py(px(1.5))
                                .rounded(px(6.0))
                                .child(t("Delete").text_color(rgb(0xffffff)).text_sm())
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        let pending = this.pending_action.take();
                                        let local_path = this.local_path.clone();
                                        let show_hidden = this.show_hidden_files;

                                        match pending {
                                            Some(PendingAction::DeleteRemote {
                                                device_id,
                                                remote_path,
                                            }) => {
                                                let result = this.runtime.block_on(
                                                    this.file_manager
                                                        .delete_file(&device_id, &remote_path),
                                                );
                                                match result {
                                                    Ok(()) => {
                                                        let refreshed = this
                                                            .runtime
                                                            .block_on(this.file_manager.list_files(
                                                                &device_id,
                                                                &remote_path,
                                                                show_hidden,
                                                            ))
                                                            .unwrap_or_default();
                                                        this.remote_files = refreshed;
                                                        this.show_confirm = false;
                                                        this.pending_action = None;
                                                        this.set_status("Deleted", false);
                                                    }
                                                    Err(e) => {
                                                        this.show_confirm = false;
                                                        this.pending_action = None;
                                                        this.set_status(
                                                            format!("Delete failed: {e}"),
                                                            true,
                                                        );
                                                    }
                                                }
                                            }
                                            Some(PendingAction::DeleteLocal { path }) => {
                                                let result = if path.is_dir() {
                                                    std::fs::remove_dir_all(&path)
                                                } else {
                                                    std::fs::remove_file(&path)
                                                };
                                                match result {
                                                    Ok(()) => {
                                                        let refreshed = this
                                                            .runtime
                                                            .block_on(
                                                                this.file_manager.list_local_files(
                                                                    &local_path,
                                                                    show_hidden,
                                                                ),
                                                            )
                                                            .unwrap_or_default();
                                                        this.local_files = refreshed;
                                                        this.show_confirm = false;
                                                        this.pending_action = None;
                                                        this.set_status("Deleted", false);
                                                    }
                                                    Err(e) => {
                                                        this.show_confirm = false;
                                                        this.pending_action = None;
                                                        this.set_status(
                                                            format!("Delete failed: {e}"),
                                                            true,
                                                        );
                                                    }
                                                }
                                            }
                                            None => {
                                                this.show_confirm = false;
                                                this.pending_action = None;
                                            }
                                        }
                                        cx.notify();
                                    }),
                                ),
                        ),
                ),
        )
}

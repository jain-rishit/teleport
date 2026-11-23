use crate::{domain::models, services};
use gpui::prelude::*;
use gpui::{Context, Render, Window, div, rgb};
use std::path::PathBuf;
use std::sync::Arc;

mod browser;
mod home;
mod settings;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewName {
    Home,
    Browser,
    Settings,
}

#[derive(Clone)]
pub(crate) enum PendingAction {
    DeleteLocal {
        path: PathBuf,
    },
    DeleteRemote {
        device_id: String,
        remote_path: String,
    },
}

pub struct AppView {
    pub(crate) devices: Vec<models::Device>,
    pub(crate) current_view: ViewName,
    pub(crate) connected_device: Option<models::Device>,
    pub(crate) local_files: Vec<models::FileEntry>,
    pub(crate) remote_files: Vec<models::FileEntry>,
    pub(crate) local_path: PathBuf,
    pub(crate) remote_path: String,
    pub(crate) browser_device_name: String,
    pub(crate) status_message: String,
    pub(crate) status_is_error: bool,
    pub(crate) is_local_loading: bool,
    pub(crate) is_remote_loading: bool,
    pub(crate) show_hidden_files: bool,
    pub(crate) default_destination: PathBuf,
    pub(crate) theme: String,
    pub(crate) is_dark_theme: bool,
    pub(crate) show_transfer_popup: bool,
    pub(crate) transfer_file_name: String,
    pub(crate) transfer_bytes: u64,
    pub(crate) transfer_total: u64,
    pub(crate) transfer_speed: String,
    pub(crate) transfer_percent: f32,
    pub(crate) transfer_complete: bool,
    pub(crate) show_confirm: bool,
    pub(crate) confirm_title: String,
    pub(crate) confirm_message: String,
    pub(crate) pending_action: Option<PendingAction>,
    pub(crate) file_manager: Arc<services::file_manager::FileManager>,
    pub(crate) device_manager: Arc<services::DeviceManager>,
    pub(crate) runtime: Arc<tokio::runtime::Runtime>,
}

impl AppView {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        let runtime =
            Arc::new(tokio::runtime::Runtime::new().expect("failed to create tokio runtime"));
        let device_manager =
            Arc::new(services::DeviceManager::new().expect("failed to create device manager"));
        let file_manager = Arc::new(services::file_manager::FileManager::new());
        let cfg = crate::application::config::Config::load().unwrap_or_default();

        // Initial device scan (synchronous USB enumeration).
        let devices = device_manager.refresh_devices().unwrap_or_default();
        let is_dark_theme = cfg.theme == "dark";

        Self {
            devices,
            current_view: ViewName::Home,
            connected_device: None,
            local_files: Vec::new(),
            remote_files: Vec::new(),
            local_path: cfg.default_download_path.clone(),
            remote_path: "/".to_string(),
            browser_device_name: String::new(),
            status_message: String::new(),
            status_is_error: false,
            is_local_loading: false,
            is_remote_loading: false,
            show_hidden_files: cfg.show_hidden_files,
            default_destination: cfg.default_download_path.clone(),
            theme: cfg.theme,
            is_dark_theme,
            show_transfer_popup: false,
            transfer_file_name: String::new(),
            transfer_bytes: 0,
            transfer_total: 0,
            transfer_speed: String::new(),
            transfer_percent: 0.0,
            transfer_complete: false,
            show_confirm: false,
            confirm_title: String::new(),
            confirm_message: String::new(),
            pending_action: None,
            file_manager,
            device_manager,
            runtime,
        }
    }

    fn set_status(&mut self, message: impl Into<String>, is_error: bool) {
        self.status_message = message.into();
        self.status_is_error = is_error;
    }
}

impl Render for AppView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bg = if self.is_dark_theme {
            rgb(0x1e1e1e)
        } else {
            rgb(0xf5f5f7)
        };
        let text = if self.is_dark_theme {
            rgb(0xf5f5f7)
        } else {
            rgb(0x1d1d1f)
        };

        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(bg)
            .text_color(text)
            .child(match self.current_view {
                ViewName::Home => home::render_home(self, cx),
                ViewName::Browser => browser::render_browser(self, cx),
                ViewName::Settings => settings::render_settings(self, cx),
            })
    }
}

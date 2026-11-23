use crate::ui::{AppView, ViewName};
use gpui::prelude::*;
use gpui::{Context, Div, MouseButton, div, px, rgb, rgba};

fn t(s: impl IntoElement) -> Div {
    div().child(s)
}

pub(crate) fn render_home(app: &mut AppView, cx: &mut Context<AppView>) -> Div {
    let primary = rgb(0x007aff);
    let dark = app.is_dark_theme;
    let text = if dark { rgb(0xf5f5f7) } else { rgb(0x1d1d1f) };
    let secondary = if dark { rgb(0xa0a0a5) } else { rgb(0x6e6e73) };
    let row = if dark { rgb(0x2c2c2e) } else { rgb(0xffffff) };
    let border = if dark {
        rgba(0xffffff14)
    } else {
        rgba(0x0000001a)
    };
    let muted = if dark { rgb(0x636366) } else { rgb(0xaeaeb2) };

    // Top bar
    let top = div()
        .flex()
        .justify_between()
        .items_center()
        .px(px(20.0))
        .py(px(12.0))
        .bg(if dark { rgb(0x2c2c2e) } else { rgb(0xffffff) })
        .border_b(px(1.0))
        .border_color(border)
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(t("Teleport").text_color(text).text_lg())
                .child(t("v0.1.0").text_color(muted).text_xs()),
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
                        .bg(primary)
                        .px(px(3.0))
                        .py(px(1.5))
                        .rounded(px(6.0))
                        .child(t("Scan for Devices").text_color(rgb(0xffffff)).text_sm())
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                let devices =
                                    this.device_manager.refresh_devices().unwrap_or_default();
                                this.devices = devices;
                                this.set_status(
                                    format!("Found {} device(s)", this.devices.len()),
                                    false,
                                );
                                cx.notify();
                            }),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(if dark { rgb(0x48484a) } else { rgb(0xe5e5ea) })
                        .px(px(3.0))
                        .py(px(1.5))
                        .rounded(px(6.0))
                        .child(t("Settings").text_color(text).text_sm())
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.current_view = ViewName::Settings;
                                cx.notify();
                            }),
                        ),
                ),
        );

    // Status
    let status = if !app.status_message.is_empty() {
        let color = if app.status_is_error {
            rgb(0xff453a)
        } else {
            rgb(0x30d158)
        };
        div()
            .flex()
            .items_center()
            .gap_2()
            .px(px(3.0))
            .py(px(2.0))
            .bg(if dark { rgb(0x2c2c2e) } else { rgb(0xffffff) })
            .border(px(1.0))
            .border_color(color)
            .rounded(px(8.0))
            .mx(px(20.0))
            .mt(px(12.0))
            .child(div().w(px(4.0)).h(px(16.0)).bg(color).rounded(px(9999.0)))
            .child(
                div()
                    .child(app.status_message.clone())
                    .text_color(text)
                    .text_sm(),
            )
    } else {
        div()
    };

    // Device list header
    let header_row = div()
        .flex()
        .items_center()
        .gap_2()
        .px(px(20.0))
        .pt(px(24.0))
        .pb(px(12.0))
        .flex_col()
        .items_start()
        .child(t("Devices").text_color(text).text_xl())
        .child(
            t("Select a device to browse storage and transfer files.")
                .text_color(secondary)
                .text_sm(),
        );

    let mut list = div()
        .flex()
        .flex_col()
        .gap_0()
        .pl(px(20.0))
        .pr(px(20.0))
        .pb(px(20.0));

    if app.devices.is_empty() {
        list = list.child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_4()
                .pt(px(24.0))
                .child(t("📱").text_xl())
                .child(t("No devices found").text_color(text).text_sm())
                .child(
                    t("Connect an Android device via USB and click Scan for Devices.")
                        .text_color(secondary)
                        .text_xs(),
                ),
        );
    } else {
        for device in &app.devices {
            let device_name = device.name.clone();
            let device_manufacturer = device.manufacturer.clone();
            let device_id = device.id.clone();

            let card = div()
                .flex()
                .items_center()
                .justify_between()
                .px(px(16.0))
                .py(px(12.0))
                .bg(row)
                .border(px(1.0))
                .border_color(border)
                .rounded(px(0.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_12()
                        .child(
                            div()
                                .w(px(40.0))
                                .h(px(40.0))
                                .bg(if dark { rgb(0x0a84ff) } else { rgb(0x007aff) })
                                .rounded(px(10.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    t(device_name.chars().take(2).collect::<String>())
                                        .text_color(rgb(0xffffff))
                                        .text_sm(),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(t(device_name.clone()).text_color(text).text_sm())
                                .child(
                                    t(format!("{} Android device", device_manufacturer))
                                        .text_color(secondary)
                                        .text_xs(),
                                )
                                .child(t(device_id.clone()).text_color(muted).text_xs()),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(primary)
                        .px(px(3.0))
                        .py(px(1.5))
                        .rounded(px(7.0))
                        .child(t("Connect").text_color(rgb(0xffffff)).text_sm())
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                let device_id = device_id.clone();
                                let device =
                                    this.devices.iter().find(|d| d.id == device_id).cloned();
                                if let Some(device) = device {
                                    this.connected_device = Some(device.clone());
                                    this.current_view = ViewName::Browser;
                                    this.browser_device_name = device.name.clone();
                                    this.local_path = this.default_destination.clone();
                                    this.remote_path = "/".to_string();
                                    this.is_local_loading = true;
                                    this.is_remote_loading = true;
                                }
                                cx.notify();

                                let dest = this.default_destination.clone();
                                let local = this
                                    .runtime
                                    .block_on(this.file_manager.list_local_files(&dest, false))
                                    .unwrap_or_default();
                                let remote = this
                                    .runtime
                                    .block_on(this.file_manager.list_files(&device_id, "/", false))
                                    .unwrap_or_default();

                                this.local_files = local;
                                this.remote_files = remote;
                                this.is_local_loading = false;
                                this.is_remote_loading = false;
                                cx.notify();
                            }),
                        ),
                );

            list = list.child(card);
        }
    }

    div()
        .flex()
        .flex_col()
        .w_full()
        .h_full()
        .child(top)
        .child(status)
        .child(header_row)
        .child(list)
}

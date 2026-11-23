use crate::ui::AppView;
use gpui::prelude::*;
use gpui::{Context, Div, MouseButton, div, px, rgb, rgba};

fn t(s: impl IntoElement) -> Div {
    div().child(s)
}

pub(crate) fn render_settings(app: &mut AppView, cx: &mut Context<AppView>) -> Div {
    let primary = rgb(0x007aff);
    let dark = app.is_dark_theme;
    let text = if dark { rgb(0xf5f5f7) } else { rgb(0x1d1d1f) };
    let surface = if dark { rgb(0x2c2c2e) } else { rgb(0xffffff) };
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
                                this.current_view = crate::ui::ViewName::Home;
                                cx.notify();
                            }),
                        ),
                )
                .child(t("Settings").text_color(text).text_lg()),
        );

    let pill = |active: bool, label: &'static str| {
        div()
            .flex()
            .items_center()
            .justify_center()
            .bg(if active {
                primary
            } else if dark {
                rgb(0x48484a)
            } else {
                rgb(0xe5e5ea)
            })
            .px(px(2.0))
            .py(px(1.0))
            .rounded(px(6.0))
            .child(
                t(label)
                    .text_color(if active {
                        rgb(0xffffff)
                    } else if dark {
                        rgb(0xf5f5f7)
                    } else {
                        rgb(0x1d1d1f)
                    })
                    .text_xs(),
            )
    };

    let theme_row = div()
        .flex()
        .justify_between()
        .items_center()
        .px(px(20.0))
        .py(px(16.0))
        .border_b(px(1.0))
        .border_color(border)
        .child(t("Theme").text_color(text).text_sm())
        .child(
            div()
                .flex()
                .gap_3()
                .child(pill(dark, "Dark").on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.theme = "dark".to_string();
                        this.is_dark_theme = true;
                        this.set_status("Theme set to dark", false);
                        cx.notify();
                    }),
                ))
                .child(pill(!dark, "Light").on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.theme = "light".to_string();
                        this.is_dark_theme = false;
                        this.set_status("Theme set to light", false);
                        cx.notify();
                    }),
                )),
        );

    let hidden_row = div()
        .flex()
        .justify_between()
        .items_center()
        .px(px(20.0))
        .py(px(16.0))
        .border_b(px(1.0))
        .border_color(border)
        .child(t("Show hidden files").text_color(text).text_sm())
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .bg(if app.show_hidden_files {
                    primary
                } else if dark {
                    rgb(0x48484a)
                } else {
                    rgb(0xe5e5ea)
                })
                .px(px(2.0))
                .py(px(1.0))
                .rounded(px(6.0))
                .child(
                    t(if app.show_hidden_files { "On" } else { "Off" })
                        .text_color(if app.show_hidden_files {
                            rgb(0xffffff)
                        } else {
                            if dark { rgb(0xf5f5f7) } else { rgb(0x1d1d1f) }
                        })
                        .text_xs(),
                )
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.show_hidden_files = !this.show_hidden_files;
                        this.set_status(
                            format!(
                                "Hidden files {}",
                                if this.show_hidden_files {
                                    "shown"
                                } else {
                                    "hidden"
                                }
                            ),
                            false,
                        );
                        cx.notify();
                    }),
                ),
        );

    let default_dest = app.default_destination.to_string_lossy().to_string();

    let dest_row = div()
        .flex()
        .flex_col()
        .gap_3()
        .px(px(20.0))
        .py(px(16.0))
        .border_b(px(1.0))
        .border_color(border)
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(t("Default download folder").text_color(text).text_sm())
                .child(t("Change").text_color(primary).text_xs()),
        )
        .child(
            div()
                .child(default_dest.clone())
                .text_color(muted)
                .text_xs(),
        )
        .child(
            div()
                .flex()
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
                        .child(t("Browse…").text_color(rgb(0xffffff)).text_sm())
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                let result = this
                                    .runtime
                                    .block_on(this.file_manager.choose_local_folder());
                                match result {
                                    Ok(Some(path)) => {
                                        this.default_destination = path.clone();
                                        this.local_path = path;
                                        this.set_status("Download folder updated", false);
                                    }
                                    Ok(None) => {
                                        // User canceled
                                    }
                                    Err(e) => {
                                        this.set_status(format!("Folder picker failed: {e}"), true);
                                    }
                                }
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
                        .child(t("Reset").text_color(text).text_sm())
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                let default = dirs::download_dir()
                                    .unwrap_or_else(|| std::path::PathBuf::from("."));
                                this.default_destination = default.clone();
                                this.local_path = default;
                                this.set_status("Download folder reset", false);
                                cx.notify();
                            }),
                        ),
                ),
        );

    div()
        .flex()
        .flex_col()
        .w_full()
        .h_full()
        .child(top)
        .child(theme_row)
        .child(hidden_row)
        .child(dest_row)
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .px(px(20.0))
                .py(px(16.0))
                .child(t("App version").text_color(text).text_sm())
                .child(t("v0.1.0").text_color(muted).text_sm()),
        )
}

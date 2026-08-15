//! The panel that shows what a manager wrote while it worked.
//!
//! A card has room for one shortened line, which is enough to see that
//! something is happening and not enough to read what it was. This keeps every
//! line whole, and keeps them after the work ends so a failure can still be
//! read back.

use gpui::*;

use crate::{app::App, styles, theme};

impl App {
    /// Whether the output for an operation is worth offering at all.
    pub fn has_output(&self, key: &str) -> bool {
        self.output_log
            .get(key)
            .is_some_and(|lines| !lines.is_empty())
    }

    pub fn output_is_open(&self, key: &str) -> bool {
        self.open_log.as_deref() == Some(key)
    }

    /// The label a card shows while it works. Once the manager has said
    /// something, it also opens what it said.
    pub fn status_pill(
        &self,
        key: &str,
        label: String,
        theme: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let pill = div()
            .flex_shrink_0()
            .px(px(14.0))
            .py(px(styles::spacing::XXS))
            .rounded(px(styles::radius::MD))
            .bg(theme.surface)
            .border_1()
            .border_color(theme.border)
            .text_size(px(styles::font_size::SMALL))
            .text_color(theme.text_muted);

        if !self.has_output(key) {
            return pill.child(label).into_any_element();
        }

        let hover = theme.hover;
        let toggling = key.to_string();
        let toggle = cx.listener(move |app, _: &ClickEvent, _window, cx| {
            cx.stop_propagation();
            app.toggle_output_log(&toggling, cx);
        });

        pill.id(SharedString::from(format!("status-{key}")))
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
            .on_click(toggle)
            .child(format!(
                "{label} {}",
                if self.output_is_open(key) {
                    "▴"
                } else {
                    "▾"
                }
            ))
            .into_any_element()
    }

    /// The open panel for one operation, titled with whose output it is.
    pub fn render_output_log(
        &self,
        key: &str,
        titled: &str,
        theme: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let border = theme.border;
        let text_muted = theme.text_muted;
        let hover = theme.hover;

        let closing = key.to_string();
        let close = cx.listener(move |app, _: &ClickEvent, _window, cx| {
            cx.stop_propagation();
            app.toggle_output_log(&closing, cx);
        });

        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .w_full()
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(px(styles::font_size::CAPTION))
                    .text_color(text_muted)
                    .child(titled.to_string()),
            )
            .child(
                div()
                    .id(SharedString::from(format!("close-output-{key}")))
                    .flex_shrink_0()
                    .px(px(styles::spacing::XS))
                    .py(px(styles::spacing::XXXS))
                    .rounded(px(styles::radius::SM))
                    .cursor_pointer()
                    .text_size(px(styles::font_size::CAPTION))
                    .text_color(text_muted)
                    .hover(move |s| s.bg(hover))
                    .on_click(close)
                    .child("Hide"),
            );

        let mut lines = div().flex().flex_col().w_full().min_w_0();
        for (at, line) in self
            .output_log
            .get(key)
            .map(|kept| kept.iter().collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .enumerate()
        {
            lines = lines.child(
                div()
                    .id(SharedString::from(format!("output-{key}-{at}")))
                    .w_full()
                    .min_w_0()
                    .font_family("monospace")
                    .text_size(px(styles::font_size::CAPTION))
                    .child(line.clone()),
            );
        }

        // Butted against the card it belongs to, so the two read as one.
        div()
            .w_full()
            .min_w_0()
            .px(px(styles::spacing::MD))
            .py(px(styles::spacing::SM))
            .rounded_b(px(styles::radius::MD))
            .border_1()
            .border_t_0()
            .border_color(border)
            .flex()
            .flex_col()
            .gap(px(styles::spacing::XS))
            .child(header)
            .child(
                // Occluded, or the wheel would pass through to the list this
                // sits in and scroll that instead of the output.
                div()
                    .id(SharedString::from(format!("output-scroll-{key}")))
                    .track_scroll(&self.output_scroll)
                    .occlude()
                    .w_full()
                    .min_w_0()
                    .max_h(px(180.0))
                    .overflow_y_scroll()
                    .child(lines),
            )
    }
}

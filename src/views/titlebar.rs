//! What aeris has to draw for itself when the compositor will not.
//!
//! GNOME and anything else refusing `xdg-decoration` hands the window back
//! undecorated, so without this there is no way to move, resize or close it.
//! The controls sit in the header that is there anyway; only the edges are
//! added on top.

use gpui::*;

use crate::{app::App, styles, theme};

/// How wide a window edge has to be to be grabbed.
const GRAB: f32 = 6.0;

/// The pointer that says which way an edge resizes.
fn cursor_for(edge: ResizeEdge) -> CursorStyle {
    match edge {
        ResizeEdge::Top => CursorStyle::ResizeUp,
        ResizeEdge::Bottom => CursorStyle::ResizeDown,
        ResizeEdge::Left => CursorStyle::ResizeLeft,
        ResizeEdge::Right => CursorStyle::ResizeRight,
        ResizeEdge::TopLeft => CursorStyle::ResizeUpLeftDownRight,
        ResizeEdge::TopRight => CursorStyle::ResizeUpRightDownLeft,
        ResizeEdge::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
        ResizeEdge::BottomRight => CursorStyle::ResizeUpLeftDownRight,
    }
}

impl App {
    /// The window controls, or nothing when the compositor draws its own.
    /// They live in the header rather than in a bar of their own, so an
    /// undecorated window costs no extra row.
    pub fn window_controls(
        &self,
        theme: &theme::Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        if !matches!(window.window_decorations(), Decorations::Client { .. }) {
            return None;
        }

        // Close alone, the way GNOME lays a titlebar out and what a window
        // most needs back when the compositor draws none. Minimising and
        // maximising are the compositor's to offer, and mean nothing at all
        // on one that tiles.
        Some(
            div()
                .flex()
                .flex_row()
                .items_center()
                .ml(px(styles::spacing::SM))
                .child(self.close_button(theme, cx)),
        )
    }

    /// Whether this window has to be moved and resized by hand.
    pub fn draws_own_decorations(window: &Window) -> bool {
        matches!(window.window_decorations(), Decorations::Client { .. })
    }

    /// The button that closes the window.
    fn close_button(&self, theme: &theme::Theme, _cx: &mut Context<Self>) -> Stateful<Div> {
        let hover = theme.hover;
        let ink = theme.text_muted;

        div()
            .id("window-close")
            .size(px(26.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(styles::radius::SM))
            .cursor_pointer()
            .hover(move |style| style.bg(hover))
            // The bar moves the window on mouse down, so a button has to say
            // the press was for it and not for the bar behind it.
            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                cx.stop_propagation();
            })
            .on_click(|_, window, _| window.remove_window())
            // Drawn from a vector rather than set from a font: the marks a
            // font offers arrive at whatever weight it drew them, which is
            // heavier than a titlebar wants.
            .child(svg().path("icons/close.svg").size(px(16.0)).text_color(ink))
    }

    /// The edges of an undecorated window, so it can still be resized.
    ///
    /// An edge the compositor has tiled is left alone: dragging it there does
    /// nothing, and offering the handle would only mislead.
    pub fn render_resize_edges(&self, window: &Window) -> Option<Div> {
        let Decorations::Client { tiling } = window.window_decorations() else {
            return None;
        };

        let grab = px(GRAB);
        // The frame holding the handles covers the whole window, so it must
        // let everything through. Only the handles themselves take the mouse,
        // or the window would have a sheet of glass over it.
        let mut edges = div().absolute().size_full();

        for (edge, tiled) in [
            (ResizeEdge::Top, tiling.top),
            (ResizeEdge::Bottom, tiling.bottom),
            (ResizeEdge::Left, tiling.left),
            (ResizeEdge::Right, tiling.right),
            (ResizeEdge::TopLeft, tiling.top || tiling.left),
            (ResizeEdge::TopRight, tiling.top || tiling.right),
            (ResizeEdge::BottomLeft, tiling.bottom || tiling.left),
            (ResizeEdge::BottomRight, tiling.bottom || tiling.right),
        ] {
            if tiled {
                continue;
            }

            let mut handle = div().absolute().occlude().cursor(cursor_for(edge));
            let corner = px(GRAB * 2.0);

            handle = match edge {
                ResizeEdge::Top => handle.top_0().left_0().right_0().h(grab),
                ResizeEdge::Bottom => handle.bottom_0().left_0().right_0().h(grab),
                ResizeEdge::Left => handle.left_0().top_0().bottom_0().w(grab),
                ResizeEdge::Right => handle.right_0().top_0().bottom_0().w(grab),
                ResizeEdge::TopLeft => handle.top_0().left_0().size(corner),
                ResizeEdge::TopRight => handle.top_0().right_0().size(corner),
                ResizeEdge::BottomLeft => handle.bottom_0().left_0().size(corner),
                ResizeEdge::BottomRight => handle.bottom_0().right_0().size(corner),
            };

            edges = edges.child(
                handle.on_mouse_down(MouseButton::Left, move |_, window, _| {
                    window.start_window_resize(edge);
                }),
            );
        }

        Some(edges)
    }
}

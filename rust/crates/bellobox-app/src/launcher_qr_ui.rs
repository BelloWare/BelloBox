//! Palette-owned, offline QR draft. Only explicit actions touch clipboard or disk.
use crate::session::{JobToken, SessionJobs};
use bello_workbench_ui::{EditorAppearance, EditorEvent, EditorView};
use gpui::{prelude::*, *};
use std::{
    collections::HashMap,
    sync::{Arc, atomic::Ordering},
};

const COMPACT_SIDE: f32 = 128.;
const CHROME_HEIGHT: f32 = 104.;

pub(crate) use crate::qr_input::PhysicalActivationKeys;
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Control {
    Enlarge,
    Paste,
    Clear,
    Save,
    Copy,
    Open,
}
struct Accepted {
    text: String,
    export: Arc<Image>,
    bytes: Arc<Vec<u8>>,
    compact: Arc<Image>,
    compact_pixels: u32,
    enlarged: Arc<Image>,
    enlarged_pixels: u32,
    enlarged_side: u32,
    modules: usize,
    dense: bool,
}
pub(crate) struct Open;
impl EventEmitter<Open> for LauncherQrPreview {}

pub(crate) struct LauncherQrPreview {
    editor: Entity<EditorView>,
    jobs: SessionJobs,
    saves: SessionJobs,
    accepted: Option<Accepted>,
    status: String,
    enlarged: bool,
    active: bool,
    retired: bool,
    save_pending: bool,
    image_copy_notice: Option<&'static str>,
    consume_release: Option<String>,
    activation_keys_down: [bool; 2],
    physical_keys: PhysicalActivationKeys,
    keyboard_save_armed: bool,
    controls: HashMap<Control, FocusHandle>,
    _subscriptions: Vec<Subscription>,
}
impl LauncherQrPreview {
    pub fn new(input: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| {
            let mut editor = EditorView::new(input, window, cx);
            let mut appearance = EditorAppearance::plain();
            appearance.font_size = 12.;
            appearance.line_height = 18.;
            editor.set_appearance(appearance, cx);
            editor
        });
        let subscription = cx.subscribe(&editor, |this, _, event, cx| {
            if matches!(event, EditorEvent::Changed) {
                this.changed(cx);
            }
        });
        let image_copy_notice = crate::qr_clipboard::for_app(cx);
        let mut this = Self {
            editor,
            jobs: SessionJobs::default(),
            saves: SessionJobs::default(),
            accepted: None,
            status: String::new(),
            enlarged: false,
            active: true,
            retired: false,
            save_pending: false,
            image_copy_notice,
            consume_release: None,
            activation_keys_down: [false; 2],
            physical_keys: PhysicalActivationKeys::default(),
            keyboard_save_armed: false,
            controls: [
                Control::Enlarge,
                Control::Paste,
                Control::Clear,
                Control::Save,
                Control::Copy,
                Control::Open,
            ]
            .into_iter()
            .map(|c| (c, cx.focus_handle()))
            .collect(),
            _subscriptions: vec![subscription],
        };
        this._subscriptions.push(cx.on_blur(
            &this.controls[&Control::Save],
            window,
            |this, _, _| {
                this.keyboard_save_armed = false;
            },
        ));
        this._subscriptions
            .push(cx.observe_window_activation(window, |this, window, _| {
                if !window.is_window_active() {
                    this.keyboard_save_armed = false;
                }
            }));
        this.changed(cx);
        this
    }
    pub fn retain_physical_keys(&mut self, keys: PhysicalActivationKeys) {
        self.keyboard_save_armed = false;
        self.physical_keys = keys;
    }
    #[cfg(test)]
    pub(crate) fn draft_editor(&self) -> Entity<EditorView> {
        self.editor.clone()
    }
    pub fn draft<'a>(&'a self, cx: &'a App) -> &'a str {
        self.editor.read(cx).text()
    }
    pub fn handoff(&self, cx: &App) -> Result<String, String> {
        bellobox_core::validate_input(self.draft(cx))?;
        Ok(self.draft(cx).to_string())
    }
    pub fn pending_dialog(&self) -> bool {
        self.save_pending
    }
    pub fn editor_focused(&self, window: &Window, cx: &App) -> bool {
        self.editor.read(cx).focus_handle(cx).is_focused(window)
    }
    pub fn composing(&self, window: &Window, cx: &App) -> bool {
        self.editor_focused(window, cx) && self.editor.read(cx).has_marked_text()
    }
    pub fn owns_focus(&self, window: &Window, cx: &App) -> bool {
        self.editor_focused(window, cx) || self.controls.values().any(|f| f.is_focused(window))
    }
    // Do not scan an over-limit editor buffer merely to choose a heading.
    fn title(&self, cx: &App) -> &'static str {
        if let Some(a) = self.current(cx) {
            if a.dense && !self.enlarged {
                "Dense code · Enlarge for detail"
            } else {
                "QR code"
            }
        } else if self.draft(cx).len() <= bellobox_core::qr::MAX_QR_BYTES
            && self.draft(cx).trim().is_empty()
        {
            "Enter text to encode"
        } else {
            "Not encodable"
        }
    }
    fn current(&self, cx: &App) -> Option<&Accepted> {
        self.accepted
            .as_ref()
            .filter(|a| a.text == self.draft(cx) && !self.retired)
    }
    pub fn height(&self) -> f32 {
        CHROME_HEIGHT + self.card_side()
    }
    fn card_side(&self) -> f32 {
        if self.enlarged {
            self.accepted
                .as_ref()
                .map(|a| a.enlarged_side as f32)
                .unwrap_or(192.)
        } else {
            COMPACT_SIDE
        }
    }
    pub fn retire(&mut self, cx: &mut Context<Self>) {
        self.retired = true;
        self.keyboard_save_armed = false;
        self.active = false;
        self.jobs.cancel();
        self.saves.cancel();
        self.accepted = None;
        cx.notify();
    }
    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        if self.active == active || self.retired {
            return;
        }
        self.active = active;
        if !active {
            self.keyboard_save_armed = false;
        }
        if active && self.current(cx).is_none() {
            self.changed(cx);
        } else if !active {
            self.jobs.cancel();
        }
    }
    fn changed(&mut self, cx: &mut Context<Self>) {
        self.keyboard_save_armed = false;
        let token = self.jobs.begin();
        self.saves.cancel();
        self.accepted = None;
        self.status.clear();
        let text = self.draft(cx);
        if let Err(error) = bellobox_core::validate_input(text) {
            self.status = format!("{error} Clear or Paste a smaller draft before opening.");
        } else if text.len() > bellobox_core::MAX_PREVIEW_BYTES {
            self.status = "Draft exceeds the 64 KB preview limit. Open to edit the complete text, or Clear/Paste to replace it.".into();
        } else if let Err(error) = crate::desktop::qr_jobs::validate(text) {
            self.status = error;
        } else if self.active && !self.retired {
            // Validation precedes the only payload clone and all encoding.
            let text = text.to_string();
            let cancel = self.jobs.cancellation();
            self.status = "Generating QR code…".into();
            let task = cx.background_executor().spawn(async move {
                if cancel.load(Ordering::Relaxed) {
                    return None;
                }
                let result = bellobox_core::qr::palette(&text);
                if cancel.load(Ordering::Relaxed) {
                    None
                } else {
                    Some((text, result))
                }
            });
            cx.spawn(async move |this, cx| {
                if let Some((text, result)) = task.await {
                    let _ = this.update(cx, |this, cx| this.publish(token, text, result, cx));
                }
            })
            .detach();
        }
        cx.notify();
    }
    fn publish(
        &mut self,
        token: JobToken,
        text: String,
        result: Result<bellobox_core::qr::Palette, String>,
        cx: &mut Context<Self>,
    ) {
        if !self.jobs.accepts(token) || !self.active || self.retired || text != self.draft(cx) {
            return;
        }
        match result {
            Ok(result) => {
                let bytes = Arc::new(result.export_png);
                self.accepted = Some(Accepted {
                    text,
                    export: Arc::new(Image::from_bytes(ImageFormat::Png, bytes.as_ref().clone())),
                    bytes,
                    compact: Arc::new(Image::from_bytes(ImageFormat::Png, result.compact_png)),
                    compact_pixels: result.compact_pixels,
                    enlarged: Arc::new(Image::from_bytes(ImageFormat::Png, result.enlarged_png)),
                    enlarged_pixels: result.enlarged_pixels,
                    enlarged_side: result.enlarged_side,
                    modules: result.modules,
                    dense: result.dense,
                });
                self.status = if self.image_copy_notice.is_some() {
                    "Full-resolution PNG ready to save."
                } else {
                    "Copy or save the full-resolution PNG."
                }
                .into();
            }
            Err(error) => {
                self.accepted = None;
                self.status = error;
            }
        }
        cx.notify();
    }
    fn enabled(&self, control: Control, cx: &App) -> bool {
        if self.retired || self.save_pending {
            return false;
        }
        match control {
            Control::Enlarge | Control::Save => self.current(cx).is_some(),
            Control::Copy => self.image_copy_notice.is_none() && self.current(cx).is_some(),
            Control::Clear => !self.draft(cx).is_empty(),
            Control::Open => bellobox_core::validate_input(self.draft(cx)).is_ok(),
            Control::Paste => true,
        }
    }
    fn act(&mut self, control: Control, window: &mut Window, cx: &mut Context<Self>) {
        if !self.enabled(control, cx) {
            return;
        }
        self.keyboard_save_armed = false;
        match control {
            Control::Enlarge => self.enlarged = !self.enlarged,
            Control::Paste => match cx.read_from_clipboard().and_then(|item| item.text()) {
                Some(text) if !text.is_empty() => match bellobox_core::validate_input(&text) {
                    Ok(()) => self.editor.update(cx, |e, cx| e.set_text(text, cx)),
                    Err(error) => self.status = error,
                },
                _ => self.status = "Clipboard has no text. The draft was kept.".into(),
            },
            Control::Clear => {
                self.editor
                    .update(cx, |e, cx| e.set_text(String::new(), cx));
                self.editor.read(cx).focus(window);
            }
            Control::Copy => {
                if let Some(image) = self.current(cx).map(|a| a.export.as_ref().clone()) {
                    cx.write_to_clipboard(ClipboardItem::new_image(&image));
                    self.status = "QR image copied.".into();
                }
            }
            Control::Save => self.save(cx),
            Control::Open => cx.emit(Open),
        }
        cx.notify();
    }
    fn save_snapshot(&self, cx: &App) -> Option<Arc<Vec<u8>>> {
        self.current(cx).map(|a| a.bytes.clone())
    }
    fn save(&mut self, cx: &mut Context<Self>) {
        if self.physical_keys.any_down() || self.activation_keys_down.iter().any(|down| *down) {
            self.keyboard_save_armed = false;
            self.status = "Release Enter/Space before opening Save.".into();
            cx.notify();
            return;
        }
        if self.save_pending {
            return;
        }
        let Some(bytes) = self.save_snapshot(cx) else {
            return;
        };
        let token = self.saves.begin();
        self.save_pending = true;
        self.status = "Choose where to save the QR image…".into();
        let blocker = crate::shutdown::block_quit(cx, "Finish or cancel the QR save first.");
        let path = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let dialog = cx.prompt_for_new_path(&path, Some("BelloBox QR.png"));
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let message = match dialog.await {
                Ok(Ok(Some(path))) => {
                    executor
                        .spawn(async move {
                            let _blocker = blocker;
                            match crate::save_new(&path, bytes.as_ref()) {
                                Ok(()) => format!("Saved {}", path.display()),
                                Err(error) => format!(
                                    "Not saved: {error}. Existing files are never overwritten."
                                ),
                            }
                        })
                        .await
                }
                Ok(Ok(None)) => "Save cancelled.".into(),
                _ => "The system save dialog could not be opened.".into(),
            };
            let _ = this.update(cx, |this, cx| {
                this.save_pending = false;
                if this.saves.accepts(token) && !this.retired {
                    this.status = message;
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn tab_order(&self, cx: &App) -> Vec<FocusHandle> {
        let mut order = Vec::new();
        if self.draft(cx).len() <= bellobox_core::MAX_PREVIEW_BYTES {
            order.push(self.editor.read(cx).focus_handle(cx));
        }
        order.extend(
            [
                Control::Enlarge,
                Control::Paste,
                Control::Clear,
                Control::Save,
                Control::Copy,
                Control::Open,
            ]
            .into_iter()
            .filter(|c| self.enabled(*c, cx))
            .map(|c| self.controls[&c].clone()),
        );
        order
    }
    /// Returns true when the preview owns the key. Editor keys intentionally
    /// propagate to the actual editor; tab/activation are consumed here.
    pub fn handle_key(
        &mut self,
        event: &KeyDownEvent,
        search: &FocusHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let activation_index = match event.keystroke.key.as_str() {
            "enter" => Some(0),
            "space" => Some(1),
            _ => None,
        };
        if let Some(index) = activation_index
            && self.consume_release.is_some()
        {
            self.activation_keys_down[index] = true;
            if self.consume_release.as_deref() != Some(event.keystroke.key.as_str()) {
                // Keep the first owner. A second activation key cancels Save;
                // neither release can open a chooser while the other is held.
                self.keyboard_save_armed = false;
            }
            window.prevent_default();
            cx.stop_propagation();
            return true;
        }
        if self.save_pending || self.composing(window, cx) {
            return true;
        }
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        if key == "tab" && !modifiers.platform && !modifiers.control && !modifiers.alt {
            // Focus notifications may coalesce an away/back traversal in one
            // frame. Retire the armed action at the navigation event itself.
            self.keyboard_save_armed = false;
            let order = self.tab_order(cx);
            let current = order.iter().position(|f| f.is_focused(window));
            let next = match (current, modifiers.shift) {
                (Some(0), true) => None,
                (Some(i), true) => Some(i - 1),
                (Some(i), false) if i + 1 < order.len() => Some(i + 1),
                (Some(_), false) => None,
                (None, true) => order.len().checked_sub(1),
                (None, false) => Some(0),
            };
            if !event.is_held {
                if let Some(i) = next {
                    order[i].focus(window);
                } else {
                    search.focus(window);
                }
            }
            window.prevent_default();
            cx.stop_propagation();
            return true;
        }
        if self.editor_focused(window, cx) {
            return true;
        }
        if matches!(key, "enter" | "space")
            && !modifiers.platform
            && !modifiers.control
            && !modifiers.alt
            && !modifiers.shift
            && let Some(control) = self
                .controls
                .iter()
                .find(|(_, f)| f.is_focused(window))
                .map(|(c, _)| *c)
        {
            self.consume_release = Some(key.into());
            self.activation_keys_down[if key == "enter" { 0 } else { 1 }] = true;
            if !event.is_held {
                if control == Control::Save {
                    // Do not hand an actively repeating Return to a newly opened
                    // native chooser: activate the modal only on physical release.
                    self.keyboard_save_armed = self.enabled(control, cx);
                } else {
                    self.act(control, window, cx);
                }
            }
            window.prevent_default();
            cx.stop_propagation();
            return true;
        }
        // Focused buttons must not leak row navigation or accidental Open.
        self.owns_focus(window, cx)
    }
    pub fn handle_key_up(
        &mut self,
        event: &KeyUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let index = match event.keystroke.key.as_str() {
            "enter" => 0,
            "space" => 1,
            _ => return false,
        };
        if self.consume_release.is_some() && self.activation_keys_down[index] {
            self.activation_keys_down[index] = false;
            let released = !self.activation_keys_down.iter().any(|down| *down);
            let save = std::mem::take(&mut self.keyboard_save_armed)
                && released
                && self.consume_release.as_deref() == Some(event.keystroke.key.as_str())
                && self.active
                && !self.retired
                && self.controls[&Control::Save].is_focused(window);
            if released {
                self.consume_release = None;
            }
            window.prevent_default();
            cx.stop_propagation();
            if save {
                self.act(Control::Save, window, cx);
            }
            true
        } else {
            false
        }
    }

    fn button(
        &self,
        id: &'static str,
        label: &'static str,
        control: Control,
        p: crate::theme::Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let enabled = self.enabled(control, cx);
        let focus = self.controls[&control].clone();
        div()
            .id(id)
            .debug_selector(move || id.into())
            .track_focus(&focus)
            .h(px(24.))
            .px(px(7.))
            .flex_none()
            .flex()
            .items_center()
            .rounded(px(5.))
            .border_1()
            .border_color(transparent_black())
            .text_size(px(11.))
            .text_color(if enabled {
                p.accent
            } else {
                p.secondary.opacity(0.5)
            })
            .when(enabled, |s| {
                s.cursor_pointer()
                    .focus(move |s| s.border_color(p.accent))
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        focus.focus(window);
                        cx.stop_propagation();
                    })
                    .on_click(cx.listener(move |this, event, window, cx| {
                        // Save's keyboard path requires a current armed key-down
                        // and release. Never replay a stale editor/search key-up
                        // through GPUI's synthetic button click after focus moves.
                        if control != Control::Save || matches!(event, ClickEvent::Mouse(_)) {
                            this.act(control, window, cx);
                        }
                        cx.stop_propagation();
                    }))
            })
            .child(label)
    }
}
impl Render for LauncherQrPreview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::for_window(window);
        if self.editor.read(cx).appearance().text != p.primary {
            self.editor.update(cx, |e, cx| {
                let mut a = e.appearance().clone();
                a.text = p.primary;
                a.caret = p.accent;
                a.selection = p.accent.opacity(0.18);
                e.set_appearance(a, cx);
            });
        }
        let accepted = self.current(cx);
        let title = self.title(cx);
        let metadata = format!(
            "{} / 2,000 UTF-8 bytes{}",
            self.draft(cx).len(),
            accepted
                .map(|a| format!(" · {} × {} modules (with quiet zone)", a.modules, a.modules))
                .unwrap_or_default()
        );
        let image = accepted.map(|a| {
            if self.enlarged {
                (a.enlarged.clone(), a.enlarged_pixels)
            } else {
                (a.compact.clone(), a.compact_pixels)
            }
        });
        let side = self.card_side();
        let fits = self.draft(cx).len() <= bellobox_core::MAX_PREVIEW_BYTES;
        let header = div()
            .h(px(24.))
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(12.))
                    .font_weight(FontWeight::MEDIUM)
                    .child(title),
            )
            .child(
                div()
                    .flex()
                    .gap(px(3.))
                    .child(self.button(
                        "launcher-qr-enlarge",
                        if self.enlarged { "Compact" } else { "Enlarge" },
                        Control::Enlarge,
                        p,
                        cx,
                    ))
                    .child(self.button("launcher-qr-paste", "Paste", Control::Paste, p, cx))
                    .child(self.button("launcher-qr-clear", "Clear", Control::Clear, p, cx)),
            );
        let card = div()
            .size(px(side))
            .flex_none()
            .bg(rgb(0xffffff))
            .rounded(px(8.))
            .relative()
            .when_some(image, |s, (image, pixels)| {
                // Snap the origin to the same 2x grid as the integer-module raster.
                let offset = ((side * 2. - pixels as f32) / 2.).floor() / 2.;
                s.child(
                    img(image)
                        .absolute()
                        .left(px(offset))
                        .top(px(offset))
                        .size(px(pixels as f32 / 2.)),
                )
            });
        let limit_notice = if self.draft(cx).len() > bellobox_core::MAX_INPUT_BYTES {
            "Draft exceeds the 500,000-byte input limit. Clear or Paste a smaller draft before opening. Nothing was truncated."
        } else {
            "Draft exceeds the preview limit. Open for the complete text, or replace it with Paste/Clear."
        };
        let editor = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(p.secondary)
                    .child("Encoded text"),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .p(px(6.))
                    .rounded(px(6.))
                    .bg(p.surface)
                    .when(fits, |s| s.child(self.editor.clone()))
                    .when(!fits, |s| s.text_size(px(11.)).child(limit_notice)),
            );
        let status = match self.image_copy_notice {
            Some(notice) => format!("{}\n{notice}", self.status),
            None => self.status.clone(),
        };
        let footer = div()
            .h(px(38.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(4.))
            .child(
                div()
                    .id("launcher-qr-status")
                    .flex_1()
                    .min_w_0()
                    .max_h(px(38.))
                    .overflow_y_scroll()
                    .text_size(px(10.))
                    .text_color(p.secondary)
                    .child(status),
            )
            .child(self.button("launcher-qr-save", "Save…", Control::Save, p, cx))
            .child(self.button("launcher-qr-copy", "Copy Image", Control::Copy, p, cx))
            .child(self.button("launcher-qr-open", "Open", Control::Open, p, cx));
        div()
            .id("launcher-qr-preview")
            .capture_any_mouse_down(cx.listener(|this, _, _, _| {
                this.keyboard_save_armed = false;
            }))
            .h(px(self.height()))
            .px(px(10.))
            .pb(px(10.))
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(header)
            .child(
                div()
                    .h(px(14.))
                    .flex_none()
                    .text_size(px(10.))
                    .text_color(p.secondary)
                    .child(metadata),
            )
            .child(
                div()
                    .h(px(side))
                    .flex_none()
                    .flex()
                    .gap(px(10.))
                    .child(card)
                    .child(editor),
            )
            .child(footer)
    }
}
#[cfg(test)]
mod tests;

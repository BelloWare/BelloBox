//! The full JSON window shares its calculation entity with its compact preview.
use super::*;
use crate::json_session::JsonSession;

impl BelloBox {
    pub(super) fn sync_json(&mut self, cx: &mut Context<Self>) {
        let Some(session) = &self.json else {
            return;
        };
        let s = session.read(cx);
        self.busy = s.busy;
        self.error = s.error.clone();
        self.status = s.status.clone();
        let output = s.output.clone();
        if self.output.read(cx).text() != output {
            self.output.update(cx, |e, cx| e.set_text(output, cx));
        }
        cx.notify();
    }
    pub(super) fn init_json(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = self.json.clone() else {
            return;
        };
        self._subscriptions
            .push(cx.observe(&session, |this, _, cx| this.sync_json(cx)));
        let owner = window.window_handle();
        // Explicit window lifetime fencing also covers retained test/entities;
        // dropping a host is not relied upon to cancel the transferred worker.
        self._subscriptions.push(cx.on_window_closed(move |cx| {
            if !cx.windows().contains(&owner) {
                session.update(cx, |s, cx| s.retire_window(cx));
            }
        }));
        self.sync_json(cx);
    }
}

pub(super) fn open(session: Entity<JsonSession>, cx: &mut App) -> Result<(), String> {
    open_with(session, cx, create_window)
}
fn create_window(
    session: Entity<JsonSession>,
    cx: &mut App,
) -> Result<gpui::WindowHandle<BelloBox>, String> {
    let input = session.read(cx).input().to_owned();
    let bounds = Bounds::centered(None, size(px(820.), px(660.)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(740.), px(560.))),
            titlebar: Some(TitlebarOptions {
                title: Some("JSON Tools — Bello Box".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        move |window, cx| {
            crate::shutdown::guard_window(window, cx);
            cx.new(|cx| {
                BelloBox::new_for_with_json("json".into(), input, Some(session), window, cx)
            })
        },
    )
    .map_err(|error| format!("Cannot open JSON Tools: {error}"))
}
fn open_with(
    session: Entity<JsonSession>,
    cx: &mut App,
    create: impl FnOnce(Entity<JsonSession>, &mut App) -> Result<gpui::WindowHandle<BelloBox>, String>,
) -> Result<(), String> {
    if crate::shutdown::requested(cx) {
        return Err("The app is closing.".into());
    }
    if !session.read(cx).can_transfer() {
        return Err(
            "This JSON preview is no longer available or its draft exceeds the input limit.".into(),
        );
    }
    // Neither failure nor a destination closed during admission consumes the
    // source session. Native activation may run callbacks before open returns.
    crate::screenshot_ui::area_navigation_changed(cx);
    let handle = create(session.clone(), cx)?;
    if crate::shutdown::requested(cx) || !cx.windows().contains(&handle.into()) {
        let _ = handle.update(cx, |_, w, _| w.remove_window());
        return Err("JSON Tools closed before the draft could be transferred.".into());
    }
    if let Err(error) = session.update(cx, |s, cx| s.transfer(cx)) {
        let _ = handle.update(cx, |_, window, _| window.remove_window());
        return Err(error);
    }
    if let Ok(mut settings) = Settings::load(&config_dir().join("settings.json")) {
        settings.explicit_open("json", launcher::category(session.read(cx).input()), now());
        let _ = settings.save(&config_dir().join("settings.json"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;

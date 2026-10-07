//! Source-shaped recording lifecycle host. The ordinary Start action is gated.
//! Feature-only injected media exercises review, never live capture or decoding.
mod controller;
#[cfg(test)]
pub(crate) mod tests;
use crate::{
    gif_converter::Converter,
    theme::{self, Palette},
};
use bello_platform::recording::RecordingHandle;
use bellobox_core::recording::session::Phase;
use gpui::{prelude::*, *};
use std::time::Duration;

pub fn open(cx: &mut App) {
    if crate::shutdown::requested(cx) {
        return;
    }
    let bounds = Bounds::centered(None, size(px(760.), px(520.)), cx);
    if let Err(error) = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(640.), px(440.))),
            titlebar: Some(TitlebarOptions {
                title: Some("Recording — Bello Box".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        |window, cx| cx.new(|cx| RecordingHost::new(window, cx)),
    ) {
        eprintln!("Cannot open recording: {error}");
    }
}
struct RecordingHost {
    controller: controller::Controller,
    review: Option<Entity<Converter>>,
    polling: bool,
    focus: FocusHandle,
    _lifetime: [Subscription; 3],
    #[cfg(feature = "recording-fixtures")]
    fixture: Option<bello_platform::recording::fixture::Case>,
}
impl RecordingHost {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let weak = cx.weak_entity();
        window.on_window_should_close(cx, move |window, cx| {
            let _ = weak.update(cx, |this, cx| this.close(cx));
            crate::shutdown::allow_close(window, cx)
        });
        let owner_window = window.window_handle();
        let weak = cx.weak_entity();
        let closed = cx.on_window_closed(move |cx| {
            if !cx.windows().contains(&owner_window) {
                let _ = weak.update(cx, |this, cx| this.close(cx));
            }
        });
        crate::shutdown::registry(cx);
        let shutdown = cx.observe_global::<crate::shutdown::Shutdown>(|this, cx| {
            if crate::shutdown::requested(cx) {
                this.close(cx);
            }
        });
        let quit = cx.on_app_quit(|this, cx| {
            // Forced/OS quit fallback only. App-controlled quits drain first.
            this.close(cx);
            async {}
        });
        cx.on_release(|this, cx| this.close(cx)).detach();
        let focus = cx.focus_handle();
        focus.focus(window);
        Self {
            controller: Default::default(),
            review: None,
            polling: false,
            focus,
            _lifetime: [closed, quit, shutdown],
            #[cfg(feature = "recording-fixtures")]
            fixture: fixture_case(),
        }
    }
    fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.review.is_some() || crate::shutdown::requested(cx) {
            return;
        }
        #[cfg(feature = "recording-fixtures")]
        let fixture = self.fixture;
        let registry = crate::shutdown::registry(cx);
        self.controller.start(|| {
            let start = || {
                #[cfg(feature = "recording-fixtures")]
                if let Some(case) = fixture {
                    return bello_platform::recording::fixture::start_injected(case);
                }
                RecordingHandle::start()
            };
            let handle = start()?;
            let control = handle.control();
            let Some(ticket) = registry.admit(move || control.close()) else {
                return Err(bello_platform::recording::RecordingError::Cancelled);
            };
            handle.retain_until_drained(ticket);
            Ok(handle)
        });
        self.drive(window, cx);
        cx.notify();
    }
    fn drive(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.polling || !self.controller.session.busy() {
            return;
        }
        self.polling = true;
        let owner_window = window.window_handle();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(30))
                    .await;
                let Ok((movie, continuing)) = this.update(cx, |this, cx| {
                    // Retirement remains observable with a retained entity after
                    // direct window removal. No window context owns this task.
                    if !cx.windows().contains(&owner_window) {
                        this.close(cx);
                    }
                    let movie = this.controller.poll();
                    let continuing = this.controller.session.busy();
                    this.polling = continuing;
                    cx.notify();
                    (movie, continuing)
                }) else {
                    break;
                };
                if let Some(movie) = movie {
                    let presented = owner_window.update(cx, |_, window, cx| {
                        let _ = this.update(cx, |this, cx| {
                            if this.controller.session.phase() == Phase::Reviewing {
                                this.review =
                                    Some(cx.new(|cx| Converter::new_recording(movie, window, cx)));
                                cx.notify();
                            }
                        });
                    });
                    if presented.is_err() {
                        let _ = this.update(cx, |this, cx| this.close(cx));
                    }
                }
                if !continuing {
                    break;
                }
            }
        })
        .detach();
    }
    fn close(&mut self, cx: &mut App) {
        self.controller.close();
        if let Some(review) = &self.review {
            review.update(cx, |review, cx| review.close(cx));
        }
    }
    fn supplied(&self) -> bool {
        #[cfg(feature = "recording-fixtures")]
        {
            self.fixture.is_some()
        }
        #[cfg(not(feature = "recording-fixtures"))]
        {
            false
        }
    }
}
#[cfg(feature = "recording-fixtures")]
fn fixture_case() -> Option<bello_platform::recording::fixture::Case> {
    use bello_platform::recording::fixture::Case;
    match std::env::var("BELLOBOX_RECORDING_FIXTURE").as_deref() {
        Ok("standard") => Some(Case::Standard),
        Ok("delayed-start") => Some(Case::DelayedStart),
        Ok("delayed-finalize") => Some(Case::DelayedFinalize),
        Ok("failure") => Some(Case::FailFinalize),
        Ok("recovery") => Some(Case::RecoverPublication),
        _ => None,
    }
}
impl Render for RecordingHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::for_window(window);
        if let Some(review) = &self.review {
            return div().size_full().child(review.clone()).into_any_element();
        }
        let phase = self.controller.session.phase();
        let can_start =
            matches!(phase, Phase::Idle | Phase::Failed) && !self.controller.session.busy();
        let can_stop = matches!(phase, Phase::Starting | Phase::Recording);
        let supplied = self.supplied();
        let content = div().flex().flex_col().gap_4()
            .child(div().text_xl().child("Recording"))
            .child(div().text_color(p.secondary).child(if supplied {
                "Injected finalized MOV and sealed known frames. No screen recording, movie decoding or audio is being tested."
            } else { "Native screen recording is unavailable in this build. Start does not capture, request permissions or create files." }))
            .child(div().h(px(180.)).rounded_lg().bg(p.well).flex().items_center().justify_center().child(match phase {
                Phase::Starting => "Starting…", Phase::Recording => if supplied { "Supplied-media session ready" } else { "Recording" },
                Phase::Finalizing => "Finishing movie…", Phase::Retiring => "Waiting for writer cleanup…", Phase::Failed => "Recording unavailable or failed",
                _ => "Ready",
            }))
            .child(div().flex().flex_wrap().gap_2()
                .child(button("record-start", "Start", can_start, p).when(can_start, |el| el.on_click(cx.listener(|this, _, window, cx| this.start(window, cx)))))
                .child(button("record-stop", "Stop", can_stop, p).when(can_stop, |el| el.on_click(cx.listener(|this, _, _, cx| { this.controller.stop(); cx.notify(); }))))
                .child(button("record-cancel", "Cancel", self.controller.session.busy(), p).when(self.controller.session.busy(), |el| el.on_click(cx.listener(|this, _, _, cx| { this.controller.cancel(); cx.notify(); })))) )
            .child(div().text_color(if phase == Phase::Failed { p.danger } else { p.secondary }).child(if supplied && phase == Phase::Recording { "Supplied-media session. Choose Stop to finish the movie.".to_string() } else { self.controller.status.clone() }))
            .child(div().text_xs().text_color(p.secondary).child("Silent movie lifecycle only. Area/window selection, countdown, pause, cursor, privacy and audio controls are not connected."));
        #[cfg(feature = "recording-fixtures")]
        let content = content.when(supplied, |el| {
            use bello_platform::recording::fixture::Case;
            el.child(
                div().flex().flex_wrap().gap_2().children(
                    [
                        ("Standard", Case::Standard),
                        ("Hold start", Case::DelayedStart),
                        ("Hold finish", Case::DelayedFinalize),
                        ("Fail finish", Case::FailFinalize),
                        ("Recovery", Case::RecoverPublication),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(index, (label, case))| {
                        button(("fixture-case", index), label, can_start, p)
                            .when(self.fixture == Some(case), |el| el.border_color(p.accent))
                            .when(can_start, |el| {
                                el.on_click(cx.listener(move |this, _, _, cx| {
                                    this.fixture = Some(case);
                                    cx.notify();
                                }))
                            })
                    }),
                ),
            )
        });
        div()
            .size_full()
            .id("recording-host")
            .overflow_y_scroll()
            .p_4()
            .bg(p.bg)
            .text_color(p.primary)
            .font_family(theme::ui_font())
            .text_sm()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    this.close(cx);
                    crate::shutdown::close_window(window, cx);
                }
            }))
            .child(content)
            .into_any_element()
    }
}
fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    enabled: bool,
    p: Palette,
) -> Stateful<Div> {
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_md()
        .border_1()
        .border_color(p.border)
        .bg(p.surface)
        .when(enabled, |el| {
            el.focusable()
                .tab_index(0)
                .focus(|style| style.border_color(p.accent))
                .cursor_pointer()
        })
        .when(!enabled, |el| el.text_color(p.secondary).opacity(0.55))
        .child(label.into())
}

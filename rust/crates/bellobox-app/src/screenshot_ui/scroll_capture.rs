//! DEBUG-only source-shaped scrolling capture fixture. Samples an app-owned page;
//! never calls screen capture, clipboard, OCR, a provider, or OS input injection.
use super::{button, theme};
use bellobox_core::screenshot::scroll::{
    Cancellation, FinalSampleSchedule, ScrollConfig, ScrollFrame, ScrollPhase, ScrollSession,
    analyze_sample, finish_scroll, synthetic_page_frame,
};
use gpui::{
    App, Bounds, Context, FocusHandle, Image, ImageFormat, KeyDownEvent, ScrollWheelEvent,
    TitlebarOptions, Window, WindowBounds, WindowOptions, div, img, prelude::*, px, size,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

const WIDTH: u32 = 420;
const HEIGHT: u32 = 320;
const PAGE_HEIGHT: u32 = 2200;

pub(super) fn open_fixture(cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(620.), px(660.)), cx);
    let _ = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(600.), px(640.))),
            titlebar: Some(TitlebarOptions {
                title: Some("Scrolling Capture · Synthetic Fixture — Bello Box".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        |window, cx| cx.new(|cx| Fixture::new(window, cx)),
    );
}
struct Fixture {
    session: ScrollSession,
    handoff_jobs: crate::session::SessionJobs,
    clock_origin: Instant,
    final_sample_schedule: FinalSampleSchedule,
    offset: u32,
    frame: ScrollFrame,
    page_image: Arc<Image>,
    preview_image: Option<Arc<Image>>,
    preview_revision: u64,
    preview_cancellation: Cancellation,
    status: String,
    focus: FocusHandle,
}
impl Fixture {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let frame = synthetic_page_frame(0, WIDTH, HEIGHT, PAGE_HEIGHT).expect("bounded fixture");
        let page_image = Arc::new(Image::from_bytes(
            ImageFormat::Png,
            frame.png().expect("bounded fixture PNG"),
        ));
        let session = ScrollSession::new(Some(frame.clone()), ScrollConfig::default())
            .expect("bounded fixture config");
        let focus = cx.focus_handle();
        window.focus(&focus);
        let weak = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            let _ = weak.update(cx, |this, _| {
                this.handoff_jobs.cancel();
                this.session.stop();
                this.preview_cancellation.cancel();
            });
            crate::shutdown::allow_close(window, cx)
        });
        cx.spawn_in(window, async |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(140))
                    .await;
                if this
                    .update_in(cx, |this, window, cx| {
                        this.tick(window, cx);
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        Self {
            session,
            handoff_jobs: crate::session::SessionJobs::default(),
            clock_origin: Instant::now(),
            final_sample_schedule: FinalSampleSchedule::default(),
            offset: 0,
            frame,
            page_image,
            preview_image: None,
            preview_revision: 0,
            preview_cancellation: Cancellation::default(),
            status: String::new(),
            focus,
        }
    }
    fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.handoff_jobs.cancel();
        self.final_sample_schedule.reset();
        if self.session.phase() == ScrollPhase::Failed {
            self.session.resume_watching();
        } else {
            self.session.start();
        }
        self.refresh_preview(window, cx);
        self.tick(window, cx);
        cx.notify();
    }
    fn scroll_by(&mut self, amount: i64, cx: &mut Context<Self>) {
        if self.session.phase() == ScrollPhase::Stitching {
            return;
        }
        let next =
            (i64::from(self.offset) + amount).clamp(0, i64::from(PAGE_HEIGHT - HEIGHT)) as u32;
        if next == self.offset {
            return;
        }
        self.offset = next;
        if let Ok(frame) = synthetic_page_frame(self.offset, WIDTH, HEIGHT, PAGE_HEIGHT) {
            if let Ok(png) = frame.png() {
                self.page_image = Arc::new(Image::from_bytes(ImageFormat::Png, png));
            }
            self.frame = frame;
        }
        cx.notify();
    }
    fn tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.session.ready_to_stitch() {
            self.stitch(window, cx);
            return;
        }
        if !self.session.can_sample()
            || (self.session.phase() == ScrollPhase::Stitching
                && !self
                    .final_sample_schedule
                    .ready(self.clock_origin.elapsed()))
        {
            return;
        }
        let job = match self.session.snapshot_for_analysis(self.frame.clone()) {
            Ok(job) => job,
            Err(error) => {
                self.status = error.to_string();
                if self.session.phase() == ScrollPhase::Stitching {
                    self.session.sample_failed();
                } else {
                    // A budget failure must not create a repeating failed sampler.
                    self.session.stop();
                }
                cx.notify();
                return;
            }
        };
        let task = cx
            .background_executor()
            .spawn(async move { analyze_sample(job) });
        cx.spawn_in(window, async move |this, cx| {
            let analyzed = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                let before = this.session.frame_count();
                let final_samples_before = this.session.final_samples_remaining();
                match this.session.accept_analysis(analyzed) {
                    Ok(true) => {
                        if this.session.phase() == ScrollPhase::Stitching
                            && final_samples_before == 2
                            && this.session.final_samples_remaining() == 1
                        {
                            // A periodic timer tick immediately after this completion
                            // must not take the second final sample too early.
                            if let Err(error) = this.final_sample_schedule.first_sample_accepted(
                                this.clock_origin.elapsed(),
                                this.session.config().sample_interval_ms,
                            ) {
                                this.status = error.to_string();
                                this.session.sample_failed();
                            }
                        }
                        if this.session.frame_count() != before {
                            this.refresh_preview(window, cx);
                        }
                        if this.session.ready_to_stitch() {
                            this.stitch(window, cx);
                        }
                    }
                    Ok(false) => return,
                    Err(error) => this.status = error.to_string(),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn finish(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.session.begin_finish().is_ok() {
            self.final_sample_schedule.reset();
            self.tick(window, cx);
            cx.notify();
        }
    }
    fn stitch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Ok(job) = self.session.finish_snapshot() else {
            return;
        };
        let task = cx
            .background_executor()
            .spawn(async move { finish_scroll(job) });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                match this.session.accept_finish(result) {
                    Ok(Some(result)) => {
                        this.preview_cancellation.cancel();
                        let token = this.handoff_jobs.begin();
                        this.status = "Preparing editor…".into();
                        let task = cx
                            .background_executor()
                            .spawn(async move { super::prepare_scroll_result(result) });
                        cx.spawn_in(window, async move |this, cx| {
                            let prepared = task.await;
                            let _ = this.update_in(cx, |this, window, cx| {
                                if !this.handoff_jobs.accepts(token) {
                                    return;
                                }
                                super::open_scroll_result(prepared, cx);
                                crate::shutdown::close_window(window, cx);
                            });
                        })
                        .detach();
                        cx.notify();
                    }
                    Ok(None) => {}
                    Err(error) => {
                        this.status = error.to_string();
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }
    fn refresh_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.preview_cancellation.cancel();
        self.preview_cancellation = Cancellation::default();
        self.preview_revision = self.preview_revision.wrapping_add(1);
        let revision = self.preview_revision;
        let preview = self.session.preview_snapshot();
        let cancel = self.preview_cancellation.clone();
        let task = cx
            .background_executor()
            .spawn(async move { preview.png(&cancel) });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, _, cx| {
                if revision != this.preview_revision || this.preview_cancellation.is_cancelled() {
                    return;
                }
                if let Ok(png) = result {
                    this.preview_image = Some(Arc::new(Image::from_bytes(ImageFormat::Png, png)));
                    cx.notify();
                }
            });
        })
        .detach();
    }
    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.handoff_jobs.cancel();
        self.final_sample_schedule.reset();
        self.session.stop();
        self.preview_cancellation.cancel();
        crate::shutdown::close_window(window, cx);
    }
}
impl Render for Fixture {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::for_window(window);
        let working = self.session.phase() == ScrollPhase::Stitching;
        let can_finish = self.session.can_finish();
        let idle = matches!(
            self.session.phase(),
            ScrollPhase::Idle | ScrollPhase::Failed
        );
        let message = self.session.message().unwrap_or("Scroll the page inside the orange frame yourself. Finish stitches everything captured so far.").to_owned();
        div()
            .size_full()
            .p(px(20.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .bg(p.bg)
            .text_color(p.primary)
            .font_family(theme::ui_font())
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "escape" => this.cancel(window, cx),
                    "down" => this.scroll_by(90, cx),
                    "up" => this.scroll_by(-90, cx),
                    _ => {}
                }
            }))
            .child(
                div()
                    .text_size(px(16.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("Scrolling Capture"),
            )
            .child(div().text_size(px(11.)).text_color(p.secondary).child(
                "Synthetic fixture · app-owned pixels only · native region capture unavailable",
            ))
            .child(
                div().flex().justify_center().child(
                    div()
                        .id("scroll-fixture-page")
                        .w(px(WIDTH as f32))
                        .h(px(HEIGHT as f32))
                        .border_2()
                        .border_color(p.accent)
                        .overflow_hidden()
                        .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                            let delta = f32::from(event.delta.pixel_delta(px(40.)).y);
                            if delta.is_finite() {
                                this.scroll_by((-delta).round() as i64, cx);
                            }
                            cx.stop_propagation();
                        }))
                        .child(
                            img(self.page_image.clone())
                                .w(px(WIDTH as f32))
                                .h(px(HEIGHT as f32)),
                        ),
                ),
            )
            .child(
                div()
                    .flex()
                    .gap(px(12.))
                    .p(px(12.))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(p.separator)
                    .bg(p.surface)
                    .child(
                        div()
                            .w(px(92.))
                            .h(px(104.))
                            .overflow_hidden()
                            .flex()
                            .items_center()
                            .justify_center()
                            .when_some(self.preview_image.clone(), |d, image| {
                                d.child(img(image).max_w(px(92.)).max_h(px(104.)))
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(div().text_size(px(12.)).child(if working {
                                "Stitching…"
                            } else {
                                "Manual · You scroll"
                            }))
                            .child(div().text_size(px(12.)).child(format!(
                                "{:.1} screens · {} px · {} / {} frames",
                                self.session.screens_captured(),
                                self.session.preview_rows(),
                                self.session.frame_count(),
                                self.session.config().max_frames
                            )))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(p.secondary)
                                    .child(message),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .when(idle, |d| {
                        d.child(
                            button(
                                "scroll-start",
                                if self.session.phase() == ScrollPhase::Failed {
                                    "Resume"
                                } else {
                                    "Start"
                                },
                                p,
                            )
                            .on_click(cx.listener(|this, _, window, cx| this.start(window, cx))),
                        )
                    })
                    .child(button("scroll-auto", "Auto-scroll · unavailable", p).opacity(0.5))
                    .child(div().flex_1())
                    .child(
                        button("scroll-cancel", "Cancel", p)
                            .on_click(cx.listener(|this, _, window, cx| this.cancel(window, cx))),
                    )
                    .child(
                        button(
                            "scroll-finish",
                            if working { "Stitching…" } else { "Finish" },
                            p,
                        )
                        .opacity(if can_finish { 1. } else { 0.5 })
                        .on_click(cx.listener(|this, _, window, cx| this.finish(window, cx))),
                    ),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(p.danger)
                    .child(self.status.clone()),
            )
    }
}

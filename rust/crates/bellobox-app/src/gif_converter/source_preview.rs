//! Paused source review shares the exact inspect/export worker admission lane.
use super::*;
use worker::{Request, Response};

#[derive(Default)]
pub(super) struct SourcePreview {
    pub image: Option<std::sync::Arc<RenderImage>>,
    pub loading: bool,
    pub requested: f64,
    pub actual: Option<f64>,
    pub completed_request: Option<f64>,
    pub pending_request: Option<f64>,
    revision: u64,
}
impl SourcePreview {
    pub fn reset(&mut self, cx: &mut App) {
        self.revision += 1;
        self.loading = false;
        self.requested = 0.;
        self.actual = None;
        self.completed_request = None;
        self.pending_request = None;
        if let Some(image) = self.image.take() {
            crate::image_disposal::drop_image(image, cx);
        }
    }
}
impl Converter {
    pub(super) fn seek_source(&mut self, seconds: f64, cx: &mut Context<Self>) {
        if self.model.busy() {
            return;
        }
        let Some(source) = self.model.selected.clone() else {
            return;
        };
        self.source_preview.revision += 1;
        self.source_preview.requested = seconds;
        self.source_preview.pending_request = Some(seconds);
        self.source_preview.loading = true;
        // Swift trim seeking updates the movie without changing the explicit
        // Movie/GIF choice or the independent exported GIF's playback state.
        self.submit(Request::Seek {
            generation: self.model.generation,
            revision: self.source_preview.revision,
            source,
            seconds,
            cancel: Default::default(),
        });
        self.drive_worker(cx);
        cx.notify();
    }
    pub(super) fn show_movie(&mut self, cx: &mut Context<Self>) {
        self.shows_result = false;
        self.preview.playing = false;
        if self.source_preview.image.is_none() {
            self.seek_source(self.model.options.trim_start, cx);
        }
        cx.notify();
    }
    pub(super) fn show_gif(&mut self, cx: &mut Context<Self>) {
        if self.model.result.is_none() {
            return;
        }
        self.shows_result = true;
        self.preview.playing = false;
        if self.preview.image.is_none() {
            self.preview_result(cx);
        }
        cx.notify();
    }
    pub(super) fn drive_worker(&mut self, cx: &mut Context<Self>) {
        if self.closed || crate::shutdown::requested(cx) {
            return;
        }
        let Some((ticket, request)) = self.worker.start() else {
            return;
        };
        let (generation, revision) = request.identity();
        let aborted = request.aborted_response();
        let Some(permit) = crate::shutdown::admit(cx, request.cancel_on_shutdown()) else {
            self.worker.close();
            self.worker.retire(ticket);
            return;
        };
        let svg_renderer = cx.svg_renderer();
        let mut task = permit.spawn(cx, move || {
            let mut response = request.run();
            let prepared = if let Response::Seek {
                result: Ok(frame), ..
            } = &mut response
            {
                Some(
                    Image::from_bytes(ImageFormat::Png, std::mem::take(&mut frame.png))
                        .to_image_data(svg_renderer)
                        .map_err(|e| e.to_string()),
                )
            } else {
                None
            };
            (response, prepared)
        });
        self.worker_completion = Some(task.close_handle());
        cx.spawn(async move |this, cx| {
            task.ready().await;
            let adopted = this
                .update(cx, |this, cx| {
                    if this.closed
                        || crate::shutdown::requested(cx)
                        || this.model.generation != generation
                        || revision.is_some_and(|revision| this.source_preview.revision != revision)
                    {
                        return false;
                    }
                    let (response, prepared) = match task.take() {
                        Some(response) => response,
                        None if task.aborted() => (aborted, None),
                        None => return false,
                    };
                    this.worker_completion = None;
                    match response {
                        Response::Inspect { generation, result }
                            if this.model.generation == generation =>
                        {
                            this.model.loaded(generation, result);
                            this.sync_trim(cx);
                            if this.model.info.is_some() {
                                this.seek_source(0., cx);
                            }
                        }
                        Response::Seek {
                            generation,
                            revision,
                            result,
                        } if this.model.generation == generation
                            && this.source_preview.revision == revision =>
                        {
                            this.source_preview.loading = false;
                            match (result, prepared) {
                                (Ok(frame), Some(Ok(image))) => {
                                    if let Some(old) = this.source_preview.image.replace(image) {
                                        crate::image_disposal::drop_image(old, cx);
                                    }
                                    this.source_preview.requested = frame.requested;
                                    this.source_preview.completed_request = Some(frame.requested);
                                    this.source_preview.pending_request = None;
                                    this.source_preview.actual = Some(frame.actual);
                                }
                                (Err(error), _) | (_, Some(Err(error))) => {
                                    this.source_preview.reset(cx);
                                    this.model.status =
                                        format!("Source preview unavailable: {error}");
                                    this.model.error = true;
                                }
                                _ => {}
                            }
                        }
                        Response::Export { generation, result }
                            if this.model.generation == generation =>
                        {
                            let success = result.is_ok();
                            this.model.finished(generation, result);
                            if success {
                                this.shows_result = true;
                                if this.recording.is_some() {
                                    this.shows_options = false;
                                }
                            }
                            // A cancelled retry preserves the previous result and
                            // restarts its paused reader, never a stale loading task.
                            if this.model.result.is_some() {
                                this.preview_result(cx);
                            }
                            // Export may preempt a seek while retaining an older
                            // image. Keep that image under a loading caption, then
                            // restore the latest intended request after physical
                            // export retirement, independently of GIF review.
                            let seek = this.source_preview.pending_request.or_else(|| {
                                this.source_preview
                                    .image
                                    .is_none()
                                    .then_some(this.model.options.trim_start)
                            });
                            if let Some(seconds) = seek {
                                this.seek_source(seconds, cx);
                            }
                        }
                        _ => {}
                    }
                    cx.notify();
                    true
                })
                .unwrap_or(false);
            if !adopted {
                task.close();
                task.ready().await; // Includes rejected-payload disposal.
            }
            let _ = this.update(cx, |this, cx| {
                this.worker.retire(ticket);
                this.worker_completion = None;
                this.drive_worker(cx);
                cx.notify();
            });
        })
        .detach();
    }
}

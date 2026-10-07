//! One-frame-ahead worker preview. Decode overlaps the current display delay.
//! Paused previews have no recurring timer and may retain one prefetched frame.
use super::*;
use bellobox_core::recording::gif::{GifError, GifExportResult, GifPreview};
use std::{sync::Arc, time::Instant};

struct PreparedFrame {
    image: Arc<RenderImage>,
    delay_centiseconds: u16,
}
pub(super) struct Preview {
    pub image: Option<Arc<RenderImage>>,
    pub playing: bool,
    pub loading: bool,
    task: Option<Task<()>>,
    cancel: Option<Arc<std::sync::atomic::AtomicBool>>,
    generation: u64,
    reader: Option<GifPreview>,
    result: Option<GifExportResult>,
    loops: bool,
    ended: bool,
    pending: Option<PreparedFrame>,
    deadline: Option<Instant>,
}
impl Preview {
    pub fn new(_cx: &mut App) -> Self {
        Self {
            image: None,
            playing: false,
            loading: false,
            task: None,
            cancel: None,
            generation: 0,
            reader: None,
            result: None,
            loops: false,
            ended: false,
            pending: None,
            deadline: None,
        }
    }
    pub fn reset(&mut self, cx: &mut App) {
        self.generation += 1;
        if let Some(cancel) = self.cancel.take() {
            cancel.store(true, Ordering::Release);
        }
        self.task = None;
        self.reader = None;
        self.result = None;
        if let Some(image) = self.image.take() {
            crate::image_disposal::drop_image(image, cx);
        }
        self.playing = false;
        self.loading = false;
        self.ended = false;
        self.pending = None;
        self.deadline = None;
    }
}
impl Converter {
    pub(super) fn preview_result(&mut self, cx: &mut Context<Self>) {
        self.preview.reset(cx);
        self.preview.result = self.model.result.clone();
        self.preview.loops = self.model.result_loops;
        self.step_preview(cx);
    }
    pub(super) fn toggle_preview(&mut self, cx: &mut Context<Self>) {
        if self.model.busy() || self.preview.result.is_none() {
            return;
        }
        self.preview.playing = !self.preview.playing;
        if self.preview.playing {
            // Resume from a held prefetched frame rather than seeking/reopening.
            self.preview.deadline = None;
            if let Some(frame) = self.preview.pending.take() {
                self.present_preview(frame, cx);
            } else {
                self.step_preview(cx);
            }
        }
        cx.notify();
    }
    fn present_preview(&mut self, frame: PreparedFrame, cx: &mut Context<Self>) {
        if let Some(old) = self.preview.image.replace(frame.image) {
            crate::image_disposal::drop_image(old, cx);
        }
        let now = cx.background_executor().now();
        self.preview.deadline =
            Some(now + Duration::from_millis(u64::from(frame.delay_centiseconds) * 10));
        if self.preview.playing {
            self.step_preview(cx);
        }
    }
    fn step_preview(&mut self, cx: &mut Context<Self>) {
        if self.preview.loading || self.closed || crate::shutdown::requested(cx) {
            return;
        }
        let Some(result) = self.preview.result.clone() else {
            return;
        };
        let generation = self.preview.generation;
        let model_generation = self.model.generation;
        let loops = self.preview.loops && self.preview.playing;
        let was_playing = self.preview.playing;
        let deadline = self.preview.deadline;
        let rewind = self.preview.ended;
        self.preview.ended = false;
        self.preview.loading = true;
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let signal = cancel.clone();
        self.preview.cancel = Some(cancel.clone());
        let svg_renderer = cx.svg_renderer();
        let Some(permit) =
            crate::shutdown::admit(cx, move || signal.store(true, Ordering::Release))
        else {
            self.preview.reset(cx);
            return;
        };
        let mut reader = self.preview.reader.take();
        let mut task = permit.spawn(cx, move || {
            let check = || {
                if cancel.load(Ordering::Acquire) {
                    Err(GifError::Cancelled)
                } else {
                    Ok(())
                }
            };
            check()?;
            let mut reader = match reader.take() {
                Some(reader) => reader,
                None => model::open_preview(&result)?,
            };
            if rewind {
                reader = reader.rewind()?;
            }
            let mut frame = reader.next_frame()?;
            if frame.is_none() && loops {
                reader = reader.rewind()?;
                frame = reader.next_frame()?;
            }
            check()?;
            let frame = frame
                .map(|frame| {
                    let image = Image::from_bytes(ImageFormat::Png, frame.png)
                        .to_image_data(svg_renderer)
                        .map_err(|error| GifError::Source(error.to_string()))?;
                    Ok::<_, GifError>(PreparedFrame {
                        image,
                        delay_centiseconds: frame.delay_centiseconds,
                    })
                })
                .transpose()?;
            check()?;
            Ok::<_, GifError>((reader, frame))
        });
        self.preview.task = Some(cx.spawn(async move |this, cx| {
            task.ready().await;
            // At most one decoded/PNG frame waits here. Late decoding presents
            // as soon as possible; it never causes catch-up bursts or a queue.
            if let Some(deadline) = deadline {
                let remaining = deadline.saturating_duration_since(cx.background_executor().now());
                if !remaining.is_zero() {
                    cx.background_executor().timer(remaining).await;
                }
            }
            task.ready().await;
            let _ = this.update(cx, |this, cx| {
                if this.preview.generation != generation
                    || this.model.generation != model_generation
                    || this.closed
                    || crate::shutdown::requested(cx)
                {
                    return;
                }
                let response = match task.take() {
                    Some(response) => response,
                    None if task.aborted() => Err(GifError::Source(
                        "Preview worker stopped unexpectedly. Try again.".into(),
                    )),
                    None => return,
                };
                this.preview.loading = false;
                this.preview.cancel = None;
                match response {
                    Ok((reader, frame)) => {
                        this.preview.reader = Some(reader);
                        if let Some(frame) = frame {
                            if was_playing && !this.preview.playing {
                                this.preview.pending = Some(frame);
                            } else {
                                this.present_preview(frame, cx);
                            }
                        } else {
                            this.preview.playing = false;
                            this.preview.ended = true;
                        }
                    }
                    Err(error) => {
                        this.preview.reset(cx);
                        this.model.status = format!("GIF saved, but preview failed: {error}");
                        this.model.error = true;
                    }
                }
                cx.notify();
            });
        }));
    }
}

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
        self.task = None;
        self.reader = None;
        self.result = None;
        if let Some(image) = self.image.take() {
            cx.drop_image(image, None);
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
            cx.drop_image(old, None);
        }
        let now = cx.background_executor().now();
        self.preview.deadline =
            Some(now + Duration::from_millis(u64::from(frame.delay_centiseconds) * 10));
        if self.preview.playing {
            self.step_preview(cx);
        }
    }
    fn step_preview(&mut self, cx: &mut Context<Self>) {
        if self.preview.loading {
            return;
        }
        let Some(result) = self.preview.result.clone() else {
            return;
        };
        let mut reader = self.preview.reader.take();
        let generation = self.preview.generation;
        let model_generation = self.model.generation;
        let loops = self.preview.loops && self.preview.playing;
        let was_playing = self.preview.playing;
        let deadline = self.preview.deadline;
        let rewind = self.preview.ended;
        self.preview.ended = false;
        self.preview.loading = true;
        let svg_renderer = cx.svg_renderer();
        self.preview.task = Some(cx.spawn(async move |this, cx| {
            let response = cx
                .background_executor()
                .spawn(async move {
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
                    Ok::<_, GifError>((reader, frame))
                })
                .await;
            // At most one decoded/PNG frame waits here. Late decoding presents
            // as soon as possible; it never causes catch-up bursts or a queue.
            if let Some(deadline) = deadline {
                let remaining = deadline.saturating_duration_since(cx.background_executor().now());
                if !remaining.is_zero() {
                    cx.background_executor().timer(remaining).await;
                }
            }
            let _ = this.update(cx, |this, cx| {
                if this.preview.generation != generation
                    || this.model.generation != model_generation
                {
                    return;
                }
                this.preview.loading = false;
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

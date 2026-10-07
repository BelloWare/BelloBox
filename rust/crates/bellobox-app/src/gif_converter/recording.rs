//! Narrow bridge from a finalized owner into the existing converter session.
use super::*;
use bello_platform::{
    movie::{MovieAsset, MovieCancellation, MovieSeek},
    recording::{FinalizedRecording, SaveControl},
};

#[derive(Clone, Debug)]
pub(super) struct OwnedRecording(pub FinalizedRecording);
impl PartialEq for OwnedRecording {
    fn eq(&self, other: &Self) -> bool {
        self.0.selected_movie() == other.0.selected_movie()
    }
}
impl OwnedRecording {
    pub fn supplied(&self) -> bool {
        #[cfg(feature = "recording-fixtures")]
        {
            self.0.known_frames().is_some()
        }
        #[cfg(not(feature = "recording-fixtures"))]
        {
            false
        }
    }
    pub fn seek(
        &self,
        seconds: f64,
        cancellation: MovieCancellation,
    ) -> Result<(f64, u32, u32, Vec<u8>), String> {
        let seek = MovieSeek::new(seconds).map_err(|e| e.to_string())?;
        self.0.verify().map_err(|e| e.to_string())?;
        cancellation.check().map_err(|e| e.to_string())?;
        #[cfg(feature = "recording-fixtures")]
        if let Some(recipe) = self.0.known_frames() {
            if !recipe.is_bound_to(&self.0) || seconds > recipe.info().duration {
                return Err("Recording fixture identity or seek changed.".into());
            }
            let index = (seconds * recipe.info().nominal_frame_rate).floor() as usize;
            let frame = recipe
                .frame(index.min(recipe.len() - 1))
                .map_err(|e| e.to_string())?;
            cancellation.check().map_err(|e| e.to_string())?;
            return Ok((
                frame.presentation_seconds,
                frame.width,
                frame.height,
                frame.rgba,
            ));
        }
        MovieAsset::open_selected(self.0.selected_movie(), cancellation)
            .and_then(|asset| asset.seek(seek))
            .map(|frame| frame.into_parts())
            .map_err(|e| e.to_string())
    }
}

#[cfg(feature = "recording-fixtures")]
pub(super) fn fixture_hold_export(
    cancel: &MovieCancellation,
) -> Result<(), bellobox_core::recording::gif::GifError> {
    // This opt-in finite delay exercises physical cancellation in supplied-media QA.
    // It is never used by native/ordinary movie export, even with the feature built.
    if let Ok(ms) = std::env::var("BELLOBOX_RECORDING_EXPORT_DELAY_MS")
        .unwrap_or_default()
        .parse::<u64>()
    {
        std::thread::sleep(Duration::from_millis(ms.min(5_000)));
    }
    cancel
        .check()
        .map_err(|_| bellobox_core::recording::gif::GifError::Cancelled)
}
#[cfg(feature = "recording-fixtures")]
pub(super) struct KnownFramesSource {
    recipe: bello_platform::recording::fixture::KnownFrames,
    index: usize,
    cancel: MovieCancellation,
}
#[cfg(feature = "recording-fixtures")]
impl KnownFramesSource {
    pub fn new(
        recipe: bello_platform::recording::fixture::KnownFrames,
        cancel: MovieCancellation,
    ) -> Self {
        Self {
            recipe,
            index: 0,
            cancel,
        }
    }
}
#[cfg(feature = "recording-fixtures")]
impl bellobox_core::recording::gif::SequentialFrameSource for KnownFramesSource {
    fn next_frame(
        &mut self,
        control: &bellobox_core::recording::gif::ExportControl,
    ) -> Result<
        Option<bellobox_core::recording::gif::DisplayRgbaFrame>,
        bellobox_core::recording::gif::GifError,
    > {
        use bellobox_core::recording::gif::{DisplayRgbaFrame, GifError};
        control.check_active()?;
        self.cancel.check().map_err(|_| GifError::Cancelled)?;
        self.recipe
            .verify()
            .map_err(|e| GifError::Source(e.to_string()))?;
        if self.index >= self.recipe.len() {
            return Ok(None);
        }
        let frame = self
            .recipe
            .frame(self.index)
            .map_err(|e| GifError::Source(e.to_string()))?;
        self.index += 1;
        DisplayRgbaFrame::new(
            frame.presentation_seconds,
            frame.width,
            frame.height,
            frame.rgba,
        )
        .map(Some)
    }
    fn finish(
        &mut self,
        control: &bellobox_core::recording::gif::ExportControl,
    ) -> Result<(), bellobox_core::recording::gif::GifError> {
        use bellobox_core::recording::gif::GifError;
        control.check_active()?;
        self.cancel.check().map_err(|_| GifError::Cancelled)?;
        self.recipe
            .verify()
            .map_err(|e| GifError::Source(e.to_string()))
    }
}
impl Converter {
    #[cfg(all(test, feature = "recording-fixtures"))]
    pub(crate) fn recording_path_for_test(&self) -> PathBuf {
        self.recording.as_ref().unwrap().0.path().to_owned()
    }
    fn cancel_movie_save(&mut self, cx: &mut Context<Self>) {
        if let Some(save) = &self.movie_save {
            self.model.status = match save.cancel() {
                bello_platform::recording::CancelOutcome::Cancelled => "Cancelling movie save…",
                bello_platform::recording::CancelOutcome::PublicationClaimed => {
                    "Save publication already started; waiting for its result…"
                }
            }
            .into();
            cx.notify();
        }
    }
    fn resume_recording_preview(&mut self, cx: &mut Context<Self>) {
        if self.model.selected.is_some()
            && self.source_preview.image.is_none()
            && !self.source_preview.loading
        {
            self.seek_source(self.model.options.trim_start, cx);
        }
    }
    fn save_movie(&mut self, cx: &mut Context<Self>) {
        if self.closed || self.model.busy() {
            return;
        }
        let Some(recording) = &self.recording else {
            return;
        };
        self.model.dialog = true;
        self.save_generation += 1;
        let ticket = self.save_generation;
        let name = recording
            .0
            .path()
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let dialog = cx.prompt_for_new_path(
            &std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            Some(&name),
        );
        cx.spawn(async move |this, cx| {
            let result = dialog.await;
            let _ = this.update(cx, |this, cx| {
                if this.save_generation != ticket || this.closed || crate::shutdown::requested(cx) {
                    return;
                }
                this.model.dialog = false;
                match result {
                    Ok(Ok(Some(path))) => this.save_movie_to(path, cx),
                    Ok(Ok(None)) => {}
                    _ => {
                        this.model.error = true;
                        this.model.status = "Could not open the movie save dialog.".into();
                    }
                }
                this.resume_recording_preview(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn save_movie_to(&mut self, destination: PathBuf, cx: &mut Context<Self>) {
        if self.closed || self.model.busy() {
            return;
        }
        let Some(movie) = self.recording.as_ref().map(|r| r.0.clone()) else {
            return;
        };
        if !destination
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("mov"))
        {
            self.model.status = "Choose a filename ending in .mov.".into();
            self.model.error = true;
            cx.notify();
            return;
        }
        let control = SaveControl::default();
        let cancel = control.clone();
        let Some(permit) = crate::shutdown::admit(cx, move || {
            cancel.cancel();
        }) else {
            return;
        };
        let worker_control = control.clone();
        let mut task = permit.spawn(cx, move || movie.save_copy(&destination, &worker_control));
        self.model.dialog = true;
        self.save_generation += 1;
        let ticket = self.save_generation;
        self.movie_save = Some(control);
        self.movie_save_completion = Some(task.close_handle());
        self.model.status = "Saving movie…".into();
        self.model.error = false;
        cx.spawn(async move |this, cx| {
            task.ready().await;
            let _ = this.update(cx, |this, cx| {
                if this.save_generation != ticket || this.closed || crate::shutdown::requested(cx) {
                    return;
                }
                let result = match task.take() {
                    Some(result) => result,
                    None if task.aborted() => Err(bello_platform::recording::RecordingError::Io),
                    None => return,
                };
                this.movie_save = None;
                this.movie_save_completion = None;
                this.model.dialog = false;
                match result {
                    Ok(path) => {
                        this.model.status = format!(
                            "Movie saved to {}.",
                            path.file_name().unwrap_or_default().to_string_lossy()
                        );
                        this.model.error = false;
                    }
                    Err(bello_platform::recording::RecordingError::Cancelled) => {
                        this.model.status =
                            "Movie save cancelled. The completed recording is kept.".into();
                        this.model.error = false;
                    }
                    Err(error) => {
                        this.model.status = format!("Could not save movie: {error}");
                        this.model.error = true;
                    }
                }
                this.resume_recording_preview(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn render_recording(
        &mut self,
        p: Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let recording = self
            .recording
            .as_ref()
            .expect("recording review requires an owner");
        let movie = &recording.0;
        let info = movie.info();
        // Media yields height before actions do; the bounded smaller viewport
        // remains scrollable for recovery messages and wrapped status text.
        let reserved = 340.
            + if recording.supplied() { 24. } else { 0. }
            + if movie.recovery_warning().is_some() {
                44.
            } else {
                0.
            }
            + if self.model.result.is_some() { 40. } else { 0. };
        let media_height = (f32::from(window.viewport_size().height) - reserved).clamp(80., 300.);
        let busy = self.model.busy();
        let status = if self.model.job.is_some() {
            self.model
                .job
                .as_ref()
                .map(|job| {
                    format!(
                        "Writing GIF · {}%",
                        job.progress.load(Ordering::Acquire) * 100 / job.planned.max(1)
                    )
                })
                .unwrap_or_default()
        } else {
            self.model.status.clone()
        };
        let caption = if self.shows_result {
            "GIF preview · starts paused".into()
        } else if self.source_preview.loading {
            "Preparing paused movie preview…".into()
        } else if let Some(actual) = self.source_preview.actual {
            format!("Movie · paused at {actual:.3} s")
        } else {
            "Movie preview unavailable. The completed movie can still be saved.".into()
        };
        div().size_full().id("recording-review").overflow_y_scroll().p_4().bg(p.bg).text_color(p.primary).font_family(theme::ui_font()).text_sm().track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" { this.close(cx); crate::shutdown::close_window(window, cx); }
            }))
            .child(div().flex().flex_col().gap_2()
                .child(div().text_xl().child(if self.model.result.is_some() { "Recording · GIF ready" } else { "Recording" }))
                .child(div().text_color(p.secondary).child(movie.path().file_name().unwrap_or_default().to_string_lossy().into_owned()))
                .when_some(movie.recovery_warning(), |el, warning| el.child(div().text_color(p.danger).child(warning)))
                .when(recording.supplied(), |el| el.child(div().text_xs().text_color(p.secondary).child("Injected valid MOV with sealed known-frame preview; no screen recording or movie decoding.")))
                .child(div().h(px(media_height)).flex_shrink_0().rounded_lg().bg(p.well).flex().flex_col().items_center().justify_center().gap_2()
                    .when_some(if self.shows_result { self.preview.image.clone() } else { self.source_preview.image.clone() }, |el, image| el.child(img(image).h(px((media_height - 30.).max(40.))).object_fit(ObjectFit::Contain)))
                    .child(div().text_xs().text_color(p.secondary).child(caption)))
                .child(div().text_color(p.secondary).child(format!("{:.1} s · {} × {} · Movie {} bytes{}", info.duration, info.display_width, info.display_height, movie.byte_len(), self.model.result.as_ref().map(|r| format!(" · GIF {} bytes", r.file_size)).unwrap_or_default())))
                .when(self.model.result.is_some(), |el| el.child(div().flex().gap_2()
                    .child(button("review-movie", "Movie", p).when(!self.shows_result, |el| el.border_color(p.accent)).on_click(cx.listener(|this, _, _, cx| this.show_movie(cx))))
                    .child(button("review-gif", "GIF", p).when(self.shows_result, |el| el.border_color(p.accent)).on_click(cx.listener(|this, _, _, cx| this.show_gif(cx))))
                    .when(self.shows_result && self.preview.image.is_some(), |el| el.child(button("review-play", if self.preview.playing { "Pause GIF" } else { "Play GIF" }, p).when(!busy, |el| el.on_click(cx.listener(|this, _, _, cx| this.toggle_preview(cx))))))))
                .child(div().flex().flex_wrap().gap_2()
                    .child(button("save-movie", "Save Movie As…", p).when(!busy, |el| el.on_click(cx.listener(|this, _, _, cx| this.save_movie(cx)))).when(busy, |el| el.opacity(0.55)))
                    .child(button("make-gif", if self.model.result.is_some() { "Another GIF…" } else { "Make GIF…" }, p).when(!busy && self.model.plan().is_some(), |el| el.on_click(cx.listener(|this, _, _, cx| { this.shows_options = true; cx.notify(); }))).when(busy || self.model.plan().is_none(), |el| el.opacity(0.55)))
                    .when(self.movie_save.is_some(), |el| el.child(button("cancel-movie-save", "Cancel Save", p).on_click(cx.listener(|this, _, _, cx| this.cancel_movie_save(cx)))))
                    .when(self.model.job.is_some(), |el| el.child(button("cancel-recording-gif", "Cancel GIF", p).on_click(cx.listener(|this, _, _, cx| { this.model.cancel(); cx.notify(); }))))
                    .child(button("reveal-movie", "Show Movie in Folder", p).on_click(cx.listener(|this, _, _, cx| { if let Some(movie) = &this.recording { cx.reveal_path(movie.0.path()); } }))))
                .child(div().text_color(if self.model.error { p.danger } else { p.secondary }).child(status))
                .child(div().text_xs().text_color(p.secondary).child("The completed movie is kept when this review closes. Saves never replace an existing file. Continuous movie playback and audio are not implemented."))).into_any_element()
    }
}

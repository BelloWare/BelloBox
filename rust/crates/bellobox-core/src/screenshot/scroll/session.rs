use super::{
    analyze::{Analysis, Identity},
    *,
};
use std::sync::atomic::AtomicU64;
static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollPhase {
    Idle,
    Watching,
    Stitching,
    Finished,
    Failed,
}
/// Window-owned state. Only immutable jobs and shared frames cross worker boundaries.
#[derive(Debug)]
pub struct ScrollSession {
    id: u64,
    generation: u64,
    operation: u64,
    phase: ScrollPhase,
    config: ScrollConfig,
    frames: Vec<ScrollFrame>,
    last_sample: Option<ScrollFrame>,
    last_footer: u32,
    preview: Vec<PreviewPiece>,
    cancellation: Cancellation,
    pending: Option<Identity>,
    finish_remaining: u8,
    finish_in_flight: bool,
    final_sample_failed: bool,
    message: Option<String>,
}
impl ScrollSession {
    pub fn new(
        initial_frame: Option<ScrollFrame>,
        config: ScrollConfig,
    ) -> ScrollResultValue<Self> {
        config.validate()?;
        if let Some(frame) = &initial_frame {
            check_budget([frame.byte_len(); 4], config.stitch.max_working_bytes)?;
        }
        let preview = initial_frame
            .as_ref()
            .map(|f| PreviewPiece {
                frame_index: 0,
                from_row: 0,
                to_row: f.height(),
            })
            .into_iter()
            .collect();
        Ok(Self {
            id: NEXT_SESSION.fetch_add(1, Ordering::Relaxed),
            generation: 0,
            operation: 0,
            phase: ScrollPhase::Idle,
            config,
            frames: initial_frame.into_iter().collect(),
            last_sample: None,
            last_footer: 0,
            preview,
            cancellation: Cancellation::default(),
            pending: None,
            finish_remaining: 0,
            finish_in_flight: false,
            final_sample_failed: false,
            message: None,
        })
    }
    pub fn phase(&self) -> ScrollPhase {
        self.phase
    }
    pub fn config(&self) -> &ScrollConfig {
        &self.config
    }
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }
    pub fn frames(&self) -> &[ScrollFrame] {
        &self.frames
    }
    pub fn preview(&self) -> &[PreviewPiece] {
        &self.preview
    }
    pub fn preview_rows(&self) -> u64 {
        self.preview.iter().map(|p| u64::from(p.rows())).sum()
    }
    pub fn screens_captured(&self) -> f64 {
        self.frames
            .first()
            .map_or(0., |f| self.preview_rows() as f64 / f64::from(f.height()))
    }
    pub fn preview_snapshot(&self) -> ScrollPreview {
        ScrollPreview {
            frames: self.frames.clone(),
            pieces: self.preview.clone(),
        }
    }
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }
    pub fn is_sample_pending(&self) -> bool {
        self.pending.is_some()
    }
    pub fn can_sample(&self) -> bool {
        self.pending.is_none()
            && !self.finish_in_flight
            && self.frames.len() < self.config.max_frames
            && (self.phase == ScrollPhase::Watching
                || (self.phase == ScrollPhase::Stitching && self.finish_remaining > 0))
    }
    pub fn can_finish(&self) -> bool {
        !self.frames.is_empty() && matches!(self.phase, ScrollPhase::Idle | ScrollPhase::Watching)
    }
    pub fn final_samples_remaining(&self) -> u8 {
        self.finish_remaining
    }
    pub fn ready_to_stitch(&self) -> bool {
        self.phase == ScrollPhase::Stitching
            && self.finish_remaining == 0
            && self.pending.is_none()
            && !self.finish_in_flight
    }
    fn invalidate(&mut self) {
        self.cancellation.cancel();
        self.cancellation = Cancellation::default();
        self.generation = self.generation.wrapping_add(1);
        self.pending = None;
        self.finish_in_flight = false;
    }
    pub fn start(&mut self) -> bool {
        if self.phase == ScrollPhase::Watching {
            return true;
        }
        if self.phase != ScrollPhase::Idle {
            return false;
        }
        self.invalidate();
        self.phase = ScrollPhase::Watching;
        true
    }
    pub fn stop(&mut self) {
        self.invalidate();
        self.finish_remaining = 0;
        if matches!(self.phase, ScrollPhase::Watching | ScrollPhase::Stitching) {
            self.phase = ScrollPhase::Idle;
        }
    }
    pub fn resume_watching(&mut self) -> bool {
        if self.phase != ScrollPhase::Failed {
            return false;
        }
        self.phase = ScrollPhase::Idle;
        self.start()
    }
    pub fn snapshot_for_analysis(&mut self, sample: ScrollFrame) -> ScrollResultValue<AnalysisJob> {
        if !self.can_sample() {
            return Err(ScrollError::InvalidState);
        }
        let retained = check_budget(
            self.frames.iter().map(ScrollFrame::byte_len),
            self.config.stitch.max_working_bytes,
        )?;
        // The caller keeps one viewport frame; reserve it and the grayscale scratch too.
        check_budget(
            [
                retained,
                self.last_sample.as_ref().map_or(0, ScrollFrame::byte_len),
                sample.byte_len(),
                sample.byte_len(),
                sample.byte_len(),
            ],
            self.config.stitch.max_working_bytes,
        )?;
        self.operation = self.operation.wrapping_add(1);
        let identity = Identity {
            session: self.id,
            generation: self.generation,
            operation: self.operation,
            frames: self.frames.len(),
        };
        self.pending = Some(identity);
        Ok(AnalysisJob {
            identity,
            sample,
            first: self.frames.first().cloned(),
            previous: self.frames.last().cloned(),
            last_sample: self.last_sample.clone(),
            previous_footer: self.last_footer,
            config: self.config.clone(),
            cancellation: self.cancellation.clone(),
        })
    }
    /// False means the result was stale. No result from another session is accepted.
    pub fn accept_analysis(&mut self, sample: AnalyzedSample) -> ScrollResultValue<bool> {
        if self.pending != Some(sample.identity)
            || sample.identity.session != self.id
            || sample.identity.generation != self.generation
            || sample.identity.frames != self.frames.len()
        {
            return Ok(false);
        }
        if self.cancellation.is_cancelled() {
            self.stop();
            return Err(ScrollError::Cancelled);
        }
        self.pending = None;
        if self.phase == ScrollPhase::Stitching {
            self.finish_remaining = self.finish_remaining.saturating_sub(1);
        }
        let analysis = match sample.result {
            Ok(value) => value,
            Err(ScrollError::Cancelled) => return Err(ScrollError::Cancelled),
            Err(error) => {
                self.sample_failed();
                return Err(error);
            }
        };
        let frame = sample.sample;
        match analysis {
            Analysis::First => {
                self.last_sample = Some(frame.clone());
                self.append(frame, 0, 0);
            }
            Analysis::SizeChanged => {
                if self.phase == ScrollPhase::Stitching {
                    self.sample_failed();
                } else {
                    self.stop();
                    self.message =
                        Some("The capture area changed size; scrolling capture stopped.".into());
                }
            }
            Analysis::Unchanged => self.last_sample = Some(frame),
            Analysis::Content {
                settled,
                header,
                footer,
                previous_footer,
                matched,
                reversed,
                difference,
            } => {
                let append = if let Some(matched) = matched {
                    let new_rows = frame
                        .height()
                        .saturating_sub(header + footer + matched.rows);
                    new_rows >= self.config.minimum_new_rows
                        && (settled
                            || f64::from(matched.rows) / f64::from(frame.height())
                                < self.config.eager_overlap_fraction)
                } else if reversed {
                    self.message = Some("Scroll down to capture more.".into());
                    false
                } else {
                    settled && difference >= self.config.unmatched_change_threshold
                };
                self.last_sample = Some(frame.clone());
                if append {
                    let rows = matched.map_or(0, |m| m.rows);
                    let slack = if matched.is_some() {
                        SEAM_SLACK.min(rows)
                    } else {
                        0
                    };
                    self.append(
                        frame,
                        header + rows - slack,
                        if matched.is_some() {
                            previous_footer + slack
                        } else {
                            0
                        },
                    );
                    self.last_footer = footer;
                    if matched.is_none() && self.frames.len() < self.config.max_frames {
                        self.message = Some("Could not match this section to the previous one. Some content may be missing or repeated; try smaller scrolls, and restart if content was skipped.".into());
                    }
                }
            }
        }
        if self.frames.len() >= self.config.max_frames {
            self.finish_remaining = 0;
        }
        Ok(true)
    }
    fn append(&mut self, frame: ScrollFrame, top: u32, trim_previous: u32) {
        if trim_previous > 0
            && let Some(previous) = self.preview.last_mut()
        {
            previous.to_row = self.frames[previous.frame_index]
                .height()
                .saturating_sub(trim_previous)
                .max(previous.from_row);
        }
        self.preview.push(PreviewPiece {
            frame_index: self.frames.len(),
            from_row: top.min(frame.height()),
            to_row: frame.height(),
        });
        self.frames.push(frame);
        self.message = if self.frames.len() >= self.config.max_frames {
            Some(format!(
                "Maximum of {} frames reached. Press Finish to stitch.",
                self.config.max_frames
            ))
        } else {
            None
        };
    }
    /// Report an acquisition failure without copying backend/path/pixel details into UI.
    pub fn sample_failed(&mut self) {
        self.pending = None;
        if self.phase == ScrollPhase::Stitching {
            self.final_sample_failed = true;
            self.finish_remaining = 0;
        } else if self.phase == ScrollPhase::Watching {
            self.message = Some("The selected area could not be sampled. You can finish the frames already captured.".into());
        }
    }
    pub fn begin_finish(&mut self) -> ScrollResultValue<()> {
        if !self.can_finish() {
            return Err(ScrollError::InvalidState);
        }
        self.invalidate();
        self.phase = ScrollPhase::Stitching;
        self.final_sample_failed = false;
        self.finish_remaining = if self.frames.len() < self.config.max_frames {
            2
        } else {
            0
        };
        Ok(())
    }
    pub fn finish_snapshot(&mut self) -> ScrollResultValue<FinishJob> {
        if !self.ready_to_stitch() {
            return Err(ScrollError::InvalidState);
        }
        self.operation = self.operation.wrapping_add(1);
        let identity = Identity {
            session: self.id,
            generation: self.generation,
            operation: self.operation,
            frames: self.frames.len(),
        };
        self.pending = Some(identity);
        self.finish_in_flight = true;
        // Last sample is no longer needed. Jobs share the retained frame allocations.
        self.last_sample = None;
        Ok(FinishJob {
            identity,
            frames: self.frames.clone(),
            config: self.config.stitch.clone(),
            cancellation: self.cancellation.clone(),
            final_sample_failed: self.final_sample_failed,
        })
    }
    pub fn accept_finish(
        &mut self,
        finished: FinishedScroll,
    ) -> ScrollResultValue<Option<ScrollResult>> {
        if self.pending != Some(finished.identity)
            || finished.identity.session != self.id
            || finished.identity.generation != self.generation
            || self.phase != ScrollPhase::Stitching
        {
            return Ok(None);
        }
        if self.cancellation.is_cancelled() {
            self.stop();
            return Err(ScrollError::Cancelled);
        }
        self.pending = None;
        self.finish_in_flight = false;
        match finished.result {
            Ok(result) => {
                self.phase = ScrollPhase::Finished;
                Ok(Some(result))
            }
            Err(ScrollError::Cancelled) => {
                self.phase = ScrollPhase::Idle;
                Err(ScrollError::Cancelled)
            }
            Err(error) => {
                self.phase = ScrollPhase::Failed;
                self.message = Some(format!("Stitching failed: {error}"));
                Err(error)
            }
        }
    }
}
impl Drop for ScrollSession {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}
#[derive(Debug)]
pub struct FinishJob {
    identity: Identity,
    frames: Vec<ScrollFrame>,
    config: StitchConfig,
    cancellation: Cancellation,
    final_sample_failed: bool,
}
impl FinishJob {
    pub fn cancellation(&self) -> Cancellation {
        self.cancellation.clone()
    }
}
#[derive(Debug)]
pub struct FinishedScroll {
    identity: Identity,
    result: ScrollResultValue<ScrollResult>,
}
pub fn finish_scroll(job: FinishJob) -> FinishedScroll {
    let result = finish(&job);
    FinishedScroll {
        identity: job.identity,
        result,
    }
}
fn finish(job: &FinishJob) -> ScrollResultValue<ScrollResult> {
    let mut count = job.frames.len();
    loop {
        job.cancellation.check()?;
        // Frames omitted for height still belong to the session and remain allocated.
        // Reserve their bytes rather than pretending the smaller slice freed them.
        let omitted = check_budget(
            job.frames[count..].iter().map(ScrollFrame::byte_len),
            job.config.max_working_bytes,
        )?;
        let mut config = job.config.clone();
        config.max_working_bytes = config
            .max_working_bytes
            .checked_sub(omitted)
            .filter(|remaining| *remaining > 0)
            .ok_or(ScrollError::MemoryLimit)?;
        match stitch(&job.frames[..count], &config, &job.cancellation) {
            Ok(mut result) => {
                let mut incomplete = Vec::new();
                if count < job.frames.len() {
                    incomplete.push(ScrollCaptureNote::DroppedFrames(job.frames.len() - count));
                }
                if job.final_sample_failed {
                    incomplete.push(ScrollCaptureNote::FinalSampleFailed);
                }
                incomplete.append(&mut result.notes);
                result.notes = incomplete;
                return Ok(result);
            }
            Err(ScrollError::OutputTooTall(_)) if count > 1 => count -= 1,
            other => return other,
        }
    }
}

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Identity {
    pub session: u64,
    pub generation: u64,
    pub operation: u64,
    pub frames: usize,
}
/// Immutable snapshot: a worker cannot accidentally compare against newer UI state.
#[derive(Debug)]
pub struct AnalysisJob {
    pub(super) identity: Identity,
    pub(super) sample: ScrollFrame,
    pub(super) first: Option<ScrollFrame>,
    pub(super) previous: Option<ScrollFrame>,
    pub(super) last_sample: Option<ScrollFrame>,
    pub(super) previous_footer: u32,
    pub(super) config: ScrollConfig,
    pub(super) cancellation: Cancellation,
}
impl AnalysisJob {
    pub fn cancellation(&self) -> Cancellation {
        self.cancellation.clone()
    }
}
#[derive(Debug)]
pub struct AnalyzedSample {
    pub(super) identity: Identity,
    pub(super) sample: ScrollFrame,
    pub(super) result: ScrollResultValue<Analysis>,
}
#[derive(Debug)]
pub(super) enum Analysis {
    First,
    SizeChanged,
    Unchanged,
    Content {
        settled: bool,
        header: u32,
        footer: u32,
        previous_footer: u32,
        matched: Option<stitch::Overlap>,
        reversed: bool,
        difference: f64,
    },
}
pub fn analyze_sample(job: AnalysisJob) -> AnalyzedSample {
    let result = analyze(&job);
    AnalyzedSample {
        identity: job.identity,
        sample: job.sample,
        result,
    }
}
fn analyze(job: &AnalysisJob) -> ScrollResultValue<Analysis> {
    let cancel = &job.cancellation;
    cancel.check()?;
    let (Some(first), Some(previous)) = (&job.first, &job.previous) else {
        return Ok(Analysis::First);
    };
    let sample = &job.sample;
    if sample.dimensions() != previous.dimensions() {
        return Ok(Analysis::SizeChanged);
    }
    let difference = stitch::difference(previous, sample, cancel)?;
    if difference <= job.config.change_threshold {
        return Ok(Analysis::Unchanged);
    }
    let settled = match &job.last_sample {
        Some(last) if last.dimensions() == sample.dimensions() => {
            stitch::difference(last, sample, cancel)? <= job.config.settle_threshold
        }
        _ => false,
    };
    let header = if job.config.stitch.remove_repeated_bars {
        stitch::sticky_band(first, previous, sample, true, cancel)?
    } else {
        0
    };
    let footer = if job.config.stitch.remove_repeated_bars {
        stitch::sticky_band(first, previous, sample, false, cancel)?
    } else {
        0
    };
    let previous_footer = if job.identity.frames == 1 {
        footer
    } else {
        job.previous_footer
    };
    let matched = stitch::overlap(
        previous,
        sample,
        &job.config.stitch,
        header,
        previous_footer,
        cancel,
    )?;
    let reversed = matched.is_none()
        && stitch::overlap(sample, previous, &job.config.stitch, header, footer, cancel)?.is_some();
    cancel.check()?;
    Ok(Analysis::Content {
        settled,
        header,
        footer,
        previous_footer,
        matched,
        reversed,
        difference,
    })
}

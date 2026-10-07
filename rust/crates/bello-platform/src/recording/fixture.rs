//! Explicit finite generated-media support, never an arbitrary-path admission.
//! Injected MOV bytes and their known-frame recipe are bound to the same retained
//! recording identity. Injected results are not native recording/decoding proof.
use super::{control::Admission, output::OutputTransaction, *};
use std::{
    io::Write,
    time::{Duration, Instant},
};

pub const WIDTH: u32 = 96;
pub const HEIGHT: u32 = 64;
pub const FRAME_COUNT: usize = 12;
pub const FPS: u32 = 10;
const MOVIE: &[u8] = include_bytes!("assets/known-rgb.mov");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Case {
    Standard,
    DelayedStart,
    DelayedFinalize,
    FailFinalize,
    RecoverPublication,
}

pub fn start_injected(case: Case) -> RecordingResult<RecordingHandle> {
    spawn(case, false)
}
#[cfg(target_os = "macos")]
pub fn start_native(case: Case) -> RecordingResult<RecordingHandle> {
    spawn(case, true)
}

fn spawn(case: Case, native: bool) -> RecordingResult<RecordingHandle> {
    let control = RecordingControl::default();
    let admission = Admission::acquire(control.clone())?;
    let worker = control.clone();
    std::thread::Builder::new()
        .name("BelloBox silent movie owner".into())
        .spawn(move || {
            // Admission is deliberately outside all file/native/callback owners.
            let owner = admission;
            let result = run(&owner, case, native, &worker);
            let event = match result {
                Ok(movie) => RecordingEvent::Finalized(movie),
                Err(RecordingError::Cancelled) => RecordingEvent::Cancelled,
                Err(error) => RecordingEvent::Failed(error),
            };
            worker.terminal(event);
            worker.retire_terminal();
        })
        .map_err(|_| RecordingError::Io)?;
    Ok(RecordingHandle { control })
}
fn run(
    admission: &Admission,
    case: Case,
    native: bool,
    control: &RecordingControl,
) -> RecordingResult<FinalizedRecording> {
    let _ = admission;
    if case == Case::DelayedStart {
        std::thread::sleep(Duration::from_secs(3));
    }
    control.check()?;
    let output = OutputTransaction::new(true)?;
    if case == Case::RecoverPublication {
        let mut previous = output::new_file(&output.destination)?;
        previous
            .write_all(b"Earlier file must survive")
            .map_err(|_| RecordingError::Io)?;
    }
    let info = if native {
        #[cfg(target_os = "macos")]
        {
            native_write(admission, &output.capture, control, case)?
        }
        #[cfg(not(target_os = "macos"))]
        {
            return Err(RecordingError::Unavailable);
        }
    } else {
        let mut file = output::new_file(&output.capture)?;
        file.write_all(MOVIE)
            .and_then(|_| file.sync_all())
            .map_err(|_| RecordingError::Io)?;
        drop(file);
        control.check()?;
        control.started();
        wait_for_stop(control)?;
        if case == Case::DelayedFinalize {
            std::thread::sleep(Duration::from_secs(5));
        }
        control.check()?;
        if case == Case::FailFinalize {
            return Err(RecordingError::NativeFailure);
        }
        fixture_info()
    };
    output.finish(info, native, !native, control)
}
fn wait_for_stop(control: &RecordingControl) -> RecordingResult<()> {
    let deadline = Instant::now() + Duration::from_secs(u64::from(MAX_RECORDING_SECONDS));
    while !control.stopped() {
        control.check()?;
        if Instant::now() >= deadline {
            return Err(RecordingError::TimedOut);
        }
        control.wait(Duration::from_millis(10));
    }
    control.check()
}
#[cfg(target_os = "macos")]
fn native_write(
    admission: &Admission,
    path: &Path,
    control: &RecordingControl,
    case: Case,
) -> RecordingResult<MovieInfo> {
    use super::validation::{Frame, Spec};
    let spec = Spec {
        width: WIDTH,
        height: HEIGHT,
        fps: FPS,
    };
    // The normal-build owner, not this fixture, owns native/pool/callback drain.
    super::macos::write(admission, path, spec, control, |writer| {
        control.started();
        for index in 0..FRAME_COUNT {
            control.check()?;
            if index > 0 && control.stopped() {
                break;
            }
            writer.append(
                Frame {
                    rgba: pixels(index),
                    stride: WIDTH as usize * 4,
                    pts: index as i64 * 60,
                    duration: 60,
                },
                control,
            )?;
            control.wait(Duration::from_millis(100));
        }
        #[cfg(test)]
        control.mark_fixture_frames_ready();
        wait_for_stop(control)?;
        if case == Case::DelayedFinalize {
            std::thread::sleep(Duration::from_secs(5));
        }
        if case == Case::FailFinalize {
            return Err(RecordingError::NativeFailure);
        }
        Ok(())
    })
}

fn fixture_info() -> MovieInfo {
    MovieInfo {
        duration: FRAME_COUNT as f64 / f64::from(FPS),
        display_width: f64::from(WIDTH),
        display_height: f64::from(HEIGHT),
        nominal_frame_rate: f64::from(FPS),
    }
}

#[derive(Clone, Debug)]
pub struct KnownFrames {
    recording: FinalizedRecording,
}
pub struct KnownFrame {
    pub presentation_seconds: f64,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
impl FinalizedRecording {
    pub fn known_frames(&self) -> Option<KnownFrames> {
        self.known_frames.then(|| KnownFrames {
            recording: self.clone(),
        })
    }
}
impl KnownFrames {
    pub fn info(&self) -> MovieInfo {
        self.recording.info()
    }
    pub fn len(&self) -> usize {
        FRAME_COUNT
    }
    pub fn is_empty(&self) -> bool {
        false
    }
    pub fn is_bound_to(&self, movie: &FinalizedRecording) -> bool {
        self.recording.selected == movie.selected
    }
    pub fn verify(&self) -> RecordingResult<()> {
        self.recording.verify()
    }
    pub fn frame(&self, index: usize) -> RecordingResult<KnownFrame> {
        self.verify()?;
        if index >= FRAME_COUNT {
            return Err(RecordingError::InvalidFrame);
        }
        Ok(KnownFrame {
            presentation_seconds: index as f64 / f64::from(FPS),
            width: WIDTH,
            height: HEIGHT,
            rgba: pixels(index),
        })
    }
}
fn pixels(index: usize) -> Vec<u8> {
    let mut output = Vec::with_capacity(WIDTH as usize * HEIGHT as usize * 4);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let rgb = if index == FRAME_COUNT - 1 {
                [255, 0, 255]
            } else if x < 8 && y < 16 {
                [255, 255, 255]
            } else if y < 8 {
                [0, index as u8 * 20, 255]
            } else if x < 48 {
                [240, 32, 16]
            } else {
                [16, 160, 48]
            };
            output.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
    }
    output
}

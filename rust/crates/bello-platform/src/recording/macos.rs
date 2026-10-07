//! Fallible normal-build AVAssetWriter owner. Every native object remains !Send
//! on its creation worker. No ScreenCaptureKit, audio or permission APIs occur.
use super::{
    control::{Admission, Callbacks},
    validation::{self, Frame, Spec, Timeline},
    *,
};
use block2::RcBlock;
use objc2::{AnyThread, rc::Retained, runtime::AnyObject};
use objc2_av_foundation::{
    AVAssetWriter, AVAssetWriterInput, AVAssetWriterStatus, AVFileTypeQuickTimeMovie,
    AVMediaTypeVideo, AVVideoCodecKey, AVVideoCodecTypeH264, AVVideoHeightKey, AVVideoWidthKey,
};
use objc2_core_foundation::CFRetained;
use objc2_core_media::{
    CMSampleBuffer, CMSampleTimingInfo, CMTime, CMVideoFormatDescription,
    CMVideoFormatDescriptionCreateForImageBuffer, kCMTimeInvalid,
};
use objc2_core_video::*;
use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL};
use std::{
    marker::PhantomData,
    ptr::{self, NonNull},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

/// The production owner boundary encloses every native object and autorelease
/// pool. The private producer cannot mint a finalized provenance token itself.
pub(super) fn write(
    _admission: &Admission,
    path: &Path,
    spec: Spec,
    control: &RecordingControl,
    append: impl FnOnce(&mut Writer) -> RecordingResult<()>,
) -> RecordingResult<MovieInfo> {
    let callbacks = Callbacks::default();
    let result = objc2::rc::autoreleasepool(|_| {
        let mut writer = Writer::new(path, spec)?;
        append(&mut writer)?;
        writer.finish(control, &callbacks)
    });
    // Dropping native references alone is insufficient: autoreleased writer or
    // block copies can survive until the entire worker pool above has drained.
    callbacks.wait();
    control.check()?;
    result
}

pub(super) struct Writer {
    writer: Retained<AVAssetWriter>,
    input: Retained<AVAssetWriterInput>,
    spec: Spec,
    output: PathBuf,
    timeline: Timeline,
    finish_started: bool,
    #[cfg(test)]
    finish_gate: Option<Arc<FinishGate>>,
    _thread_bound: PhantomData<Rc<()>>,
}
impl Writer {
    pub fn new(path: &Path, spec: Spec) -> RecordingResult<Self> {
        spec.validate()?;
        let url = NSURL::from_file_path(path).ok_or(RecordingError::Io)?;
        let file_type = unsafe { AVFileTypeQuickTimeMovie }.ok_or(RecordingError::Unavailable)?;
        let writer = unsafe {
            AVAssetWriter::initWithURL_fileType_error(AVAssetWriter::alloc(), &url, file_type)
        }
        .map_err(|_| RecordingError::NativeFailure)?;
        let width = NSNumber::new_u32(spec.width);
        let height = NSNumber::new_u32(spec.height);
        let keys = unsafe { [AVVideoCodecKey, AVVideoWidthKey, AVVideoHeightKey] };
        let [codec_key, width_key, height_key] =
            keys.map(|key| key.ok_or(RecordingError::Unavailable));
        let codec = unsafe { AVVideoCodecTypeH264 }.ok_or(RecordingError::Unavailable)?;
        let media_type = unsafe { AVMediaTypeVideo }.ok_or(RecordingError::Unavailable)?;
        let settings: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::from_slices(
            &[codec_key?, width_key?, height_key?],
            &[codec, &*width, &*height],
        );
        if !unsafe { writer.canApplyOutputSettings_forMediaType(Some(&settings), media_type) } {
            return Err(RecordingError::NativeFailure);
        }
        let input = unsafe {
            AVAssetWriterInput::initWithMediaType_outputSettings(
                AVAssetWriterInput::alloc(),
                media_type,
                Some(&settings),
            )
        };
        unsafe {
            input.setExpectsMediaDataInRealTime(true);
            if !writer.canAddInput(&input) {
                return Err(RecordingError::NativeFailure);
            }
            writer.addInput(&input);
        }
        let owner = Self {
            writer,
            input,
            spec,
            output: path.to_path_buf(),
            timeline: Timeline::default(),
            finish_started: false,
            #[cfg(test)]
            finish_gate: None,
            _thread_bound: PhantomData,
        };
        if !unsafe { owner.writer.startWriting() } {
            return Err(RecordingError::NativeFailure);
        }
        unsafe {
            owner
                .writer
                .startSessionAtSourceTime(CMTime::new(0, validation::TIMESCALE));
        }
        Ok(owner)
    }
    #[cfg(test)]
    pub fn hold_completion_for_test(&mut self, gate: Arc<FinishGate>) {
        self.finish_gate = Some(gate);
    }
    /// At most one RGBA frame, one CV buffer and one CMSampleBuffer are locally
    /// retained. Framework codec allocations are additional, not a claimed RSS cap.
    pub fn append(&mut self, frame: Frame, control: &RecordingControl) -> RecordingResult<()> {
        control.check()?;
        self.timeline.validate(self.spec, &frame)?;
        let deadline = Instant::now() + Duration::from_secs(10);
        while !unsafe { self.input.isReadyForMoreMediaData() } {
            control.check()?;
            if unsafe { self.writer.status() } != AVAssetWriterStatus::Writing {
                return Err(RecordingError::NativeFailure);
            }
            if Instant::now() >= deadline {
                return Err(RecordingError::TimedOut);
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        let pixel = pixel_buffer(self.spec, &frame)?;
        let mut format: *const CMVideoFormatDescription = ptr::null();
        let status = unsafe {
            CMVideoFormatDescriptionCreateForImageBuffer(None, &pixel, NonNull::from(&mut format))
        };
        if status != 0 {
            return Err(RecordingError::NativeFailure);
        }
        let format = unsafe {
            CFRetained::from_raw(
                NonNull::new(format.cast_mut()).ok_or(RecordingError::NativeFailure)?,
            )
        };
        let mut timing = CMSampleTimingInfo {
            duration: unsafe { CMTime::new(frame.duration, validation::TIMESCALE) },
            presentationTimeStamp: unsafe { CMTime::new(frame.pts, validation::TIMESCALE) },
            decodeTimeStamp: unsafe { kCMTimeInvalid },
        };
        let mut sample: *mut CMSampleBuffer = ptr::null_mut();
        let status = unsafe {
            CMSampleBuffer::create_ready_with_image_buffer(
                None,
                &pixel,
                &format,
                NonNull::from(&mut timing),
                NonNull::from(&mut sample),
            )
        };
        if status != 0 {
            return Err(RecordingError::NativeFailure);
        }
        let sample = unsafe {
            CFRetained::from_raw(NonNull::new(sample).ok_or(RecordingError::NativeFailure)?)
        };
        control.check()?;
        if !unsafe { self.input.appendSampleBuffer(&sample) } {
            return Err(RecordingError::NativeFailure);
        }
        // Sampled disk bound; AVFoundation can buffer additional opaque data.
        // The complete file is checked again before finalized ownership transfer.
        if std::fs::metadata(&self.output)
            .is_ok_and(|metadata| metadata.len() > MAX_RECORDING_FILE_BYTES)
        {
            return Err(RecordingError::LimitExceeded);
        }
        self.timeline.appended(&frame);
        Ok(())
    }
    /// Caller MUST leave its autorelease pool, then wait for callbacks to drain
    /// before file cleanup/publication or physical admission release.
    pub fn finish(
        &mut self,
        control: &RecordingControl,
        callbacks: &Callbacks,
    ) -> RecordingResult<MovieInfo> {
        control.check()?;
        if self.timeline.count() == 0 {
            return Err(RecordingError::NoFrames);
        }
        if self.finish_started || unsafe { self.writer.status() } != AVAssetWriterStatus::Writing {
            return Err(RecordingError::NativeFailure);
        }
        unsafe {
            self.input.markAsFinished();
        }
        self.finish_started = true;
        let done = Arc::new(AtomicBool::new(false));
        let notify = done.clone();
        let ticket = callbacks.ticket();
        #[cfg(test)]
        let gate = self.finish_gate.clone();
        let callback = move || {
            let _ = &ticket;
            notify.store(true, Ordering::Release);
            #[cfg(test)]
            if let Some(gate) = &gate {
                gate.hold();
            }
        };
        fn sendable<F: Send + Sync>(_: &F) {}
        sendable(&callback);
        let block = RcBlock::new(callback);
        unsafe {
            self.writer.finishWritingWithCompletionHandler(&block);
        }
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut timed_out = false;
        // Cancellation cannot cancelWriting after finish submission. Logical
        // expiry does not abandon native work or release the physical lease.
        while !done.load(Ordering::Acquire) {
            timed_out |= Instant::now() >= deadline;
            std::thread::sleep(Duration::from_millis(2));
        }
        drop(block);
        control.check()?;
        if timed_out {
            return Err(RecordingError::TimedOut);
        }
        if unsafe { self.writer.status() } != AVAssetWriterStatus::Completed {
            return Err(RecordingError::NativeFailure);
        }
        Ok(MovieInfo {
            duration: self.timeline.end as f64 / f64::from(validation::TIMESCALE),
            display_width: f64::from(self.spec.width),
            display_height: f64::from(self.spec.height),
            nominal_frame_rate: f64::from(self.spec.fps),
        })
    }
}
impl Drop for Writer {
    fn drop(&mut self) {
        if !self.finish_started
            && matches!(
                unsafe { self.writer.status() },
                AVAssetWriterStatus::Unknown | AVAssetWriterStatus::Writing
            )
        {
            unsafe {
                self.writer.cancelWriting();
            }
        }
    }
}

fn pixel_buffer(spec: Spec, frame: &Frame) -> RecordingResult<CFRetained<CVPixelBuffer>> {
    let mut pointer = ptr::null_mut();
    let status = unsafe {
        CVPixelBufferCreate(
            None,
            spec.width as usize,
            spec.height as usize,
            kCVPixelFormatType_32BGRA,
            None,
            NonNull::from(&mut pointer),
        )
    };
    if status != kCVReturnSuccess {
        return Err(RecordingError::NativeFailure);
    }
    let pixel = unsafe {
        CFRetained::from_raw(NonNull::new(pointer).ok_or(RecordingError::NativeFailure)?)
    };
    if CVPixelBufferGetWidth(&pixel) != spec.width as usize
        || CVPixelBufferGetHeight(&pixel) != spec.height as usize
        || CVPixelBufferIsPlanar(&pixel)
        || CVPixelBufferGetPixelFormatType(&pixel) != kCVPixelFormatType_32BGRA
    {
        return Err(RecordingError::InvalidFrame);
    }
    let status = unsafe { CVPixelBufferLockBaseAddress(&pixel, CVPixelBufferLockFlags::empty()) };
    if status != kCVReturnSuccess {
        return Err(RecordingError::NativeFailure);
    }
    struct Locked<'a>(&'a CVPixelBuffer, bool);
    impl Drop for Locked<'_> {
        fn drop(&mut self) {
            if self.1 {
                unsafe {
                    CVPixelBufferUnlockBaseAddress(self.0, CVPixelBufferLockFlags::empty());
                }
            }
        }
    }
    let mut lock = Locked(&pixel, true);
    let stride = CVPixelBufferGetBytesPerRow(&pixel);
    validation::validate_native_storage(spec, stride, CVPixelBufferGetDataSize(&pixel))?;
    let base = CVPixelBufferGetBaseAddress(&pixel);
    if base.is_null() {
        return Err(RecordingError::InvalidFrame);
    }
    for y in 0..spec.height as usize {
        let source = &frame.rgba[y * frame.stride..y * frame.stride + spec.width as usize * 4];
        let target =
            unsafe { std::slice::from_raw_parts_mut(base.cast::<u8>().add(y * stride), stride) };
        target.fill(0);
        for (rgba, bgra) in source.chunks_exact(4).zip(target.chunks_exact_mut(4)) {
            bgra.copy_from_slice(&[rgba[2], rgba[1], rgba[0], rgba[3]]);
        }
    }
    let status = unsafe { CVPixelBufferUnlockBaseAddress(&pixel, CVPixelBufferLockFlags::empty()) };
    lock.1 = false;
    if status != kCVReturnSuccess {
        return Err(RecordingError::NativeFailure);
    }
    drop(lock);
    Ok(pixel)
}

// Holds the actual finish completion body, not a substitute native writer.
// Only pointer-free test synchronization is captured by the copied block.
#[cfg(test)]
pub(super) struct FinishGate {
    called: std::sync::mpsc::SyncSender<()>,
    released: (std::sync::Mutex<bool>, std::sync::Condvar),
}
#[cfg(test)]
impl FinishGate {
    pub fn new() -> (Arc<Self>, std::sync::mpsc::Receiver<()>) {
        let (called, received) = std::sync::mpsc::sync_channel(1);
        (
            Arc::new(Self {
                called,
                released: (std::sync::Mutex::new(false), std::sync::Condvar::new()),
            }),
            received,
        )
    }
    fn hold(&self) {
        let _ = self.called.try_send(());
        let guard = self
            .released
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        drop(
            self.released
                .1
                .wait_timeout_while(guard, Duration::from_secs(5), |released| !*released)
                .unwrap_or_else(|error| error.into_inner()),
        );
    }
    pub fn release(&self) {
        *self
            .released
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = true;
        self.released.1.notify_all();
    }
}

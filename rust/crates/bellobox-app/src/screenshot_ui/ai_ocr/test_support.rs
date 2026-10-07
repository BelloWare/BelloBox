//! Deterministic holds around actual completed work. These never substitute an
//! OCR result or bypass the real preparation/HTTP/parser/controller path.
use super::*;
use std::{
    future::poll_fn,
    sync::Mutex,
    task::{Poll, Waker},
};

pub(in crate::screenshot_ui) static TEST_LOCK: Mutex<()> = Mutex::new(());

#[derive(Default)]
pub(in crate::screenshot_ui) struct TestPause {
    released: AtomicBool,
    waiting: AtomicBool,
    waker: Mutex<Option<Waker>>,
}
impl TestPause {
    pub fn is_waiting(&self) -> bool {
        self.waiting.load(Ordering::Acquire)
    }
    pub fn release(&self) {
        self.released.store(true, Ordering::Release);
        if let Some(waker) = self.waker.lock().unwrap().take() {
            waker.wake();
        }
    }
    pub(super) async fn wait(&self) {
        poll_fn(|cx| {
            let mut waker = self.waker.lock().unwrap();
            if self.released.load(Ordering::Acquire) {
                return Poll::Ready(());
            }
            *waker = Some(cx.waker().clone());
            self.waiting.store(true, Ordering::Release);
            Poll::Pending
        })
        .await;
    }
}
impl ScreenshotEditor {
    pub(in crate::screenshot_ui) fn ai_test_cancel_before_count(
        &mut self,
        cancellation: Arc<AtomicBool>,
    ) {
        self.ai_ocr.cancel_before_count = Some(cancellation);
    }
    pub(in crate::screenshot_ui) fn ai_test_owned_admission_available(&self, cx: &mut App) -> bool {
        self.owned_inline_work(cx).is_ok()
    }
    pub(in crate::screenshot_ui) fn ai_test_pause_preparation(&mut self) -> Arc<TestPause> {
        let pause = Arc::new(TestPause::default());
        self.ai_ocr.preparation_pause = Some(pause.clone());
        pause
    }
    pub(in crate::screenshot_ui) fn ai_test_pause_completion(&mut self) -> Arc<TestPause> {
        let pause = Arc::new(TestPause::default());
        self.ai_ocr.completion_pause = Some(pause.clone());
        pause
    }
    pub(in crate::screenshot_ui) fn ai_test_pause_disposal(&mut self) -> Arc<TestPause> {
        let pause = Arc::new(TestPause::default());
        self.ai_ocr.disposal_pause = Some(pause.clone());
        pause
    }
    pub(in crate::screenshot_ui) fn ai_test_pause_save(&mut self) -> Arc<TestPause> {
        let pause = Arc::new(TestPause::default());
        self.ai_ocr.save_pause = Some(pause.clone());
        pause
    }
    pub(in crate::screenshot_ui) fn ai_test_permit(&self) -> transport::fixture::FixturePermit {
        self.ai_ocr
            .fixture
            .as_ref()
            .expect("supplied fixture")
            .permit
            .clone()
    }
}

pub(in crate::screenshot_ui) struct HeldSave {
    pub entered: std::sync::mpsc::Receiver<()>,
    pub release: std::sync::mpsc::Sender<()>,
    pub result: std::sync::mpsc::Receiver<Result<(), String>>,
    pub finished: std::sync::mpsc::Receiver<()>,
}
impl ScreenshotEditor {
    /// Same physical save owner and writer as the actual chooser dispatch. The
    /// test hold follows a real write+sync of its private staging file, before
    /// cancellation checks and publication. Dropping result models a lost UI
    /// awaiter while the synchronous file worker is still physically executing.
    pub(in crate::screenshot_ui) fn ai_test_held_save(
        &mut self,
        path: std::path::PathBuf,
        cx: &mut Context<Self>,
    ) -> HeldSave {
        let text = self
            .reader_text(false)
            .expect("current supplied OCR result")
            .to_owned();
        let authority = self
            .ai_ocr
            .result
            .as_ref()
            .map(|result| result.provider.clone());
        let inline_work = self
            .inline
            .as_ref()
            .and_then(|inline| main_area::begin_owned_editor_work(inline.id, cx));
        self.ai_ocr.save_jobs.begin();
        let cancel = self.ai_ocr.save_jobs.cancellation();
        let owned = OwnedSave {
            text,
            authority,
            inline_work,
        };
        let (entered_tx, entered) = std::sync::mpsc::channel();
        let (release, released) = std::sync::mpsc::channel();
        let (result_tx, result) = std::sync::mpsc::channel();
        let (finished_tx, finished) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let value = run_owned_save(&path, owned, &cancel, || {
                entered_tx.send(()).unwrap();
                released.recv().unwrap();
            });
            let _ = result_tx.send(value);
            let _ = finished_tx.send(());
        });
        HeldSave {
            entered,
            release,
            result,
            finished,
        }
    }
}

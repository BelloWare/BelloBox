//! Explicit-action setup state. A cancelled blocking worker keeps its admission
//! lease until it physically exits, even after the Settings window is dropped.
use bellobox_core::ai::{
    Config,
    provider_setup::{models_request, test_request},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

#[cfg(test)]
pub(super) static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

static WORKER_BUSY: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Load,
    Test,
}

enum Reply {
    Models(Vec<String>),
    Tested(String),
}
struct Pending {
    generation: u64,
    cancel: Arc<AtomicBool>,
    receive: mpsc::Receiver<Result<Reply, String>>,
}
struct WorkerLease;
impl WorkerLease {
    fn acquire() -> Result<Self, String> {
        WORKER_BUSY
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Self)
            .map_err(|_| {
                "A provider setup request is still stopping. Retry when it finishes.".into()
            })
    }
}
impl Drop for WorkerLease {
    fn drop(&mut self) {
        WORKER_BUSY.store(false, Ordering::Release);
    }
}

#[derive(Default)]
pub(super) struct Setup {
    generation: u64,
    closed: bool,
    #[cfg(test)]
    pub(super) fail_next_spawn: bool,
    pending: Option<Pending>,
    pub models: Vec<String>,
    pub load_message: Option<String>,
    pub test_message: Option<String>,
    pub action: Option<Action>,
}
impl Setup {
    pub fn close(&mut self) {
        self.closed = true;
        self.invalidate(true);
    }
    pub fn invalidate(&mut self, clear_models: bool) {
        self.generation = self.generation.wrapping_add(1);
        if let Some(pending) = &self.pending {
            pending.cancel.store(true, Ordering::Release);
        }
        if clear_models {
            self.models.clear();
        }
        self.load_message = None;
        self.test_message = None;
        self.action = None;
    }
    #[cfg(test)]
    pub(super) fn delayed_fixture(&mut self, action: Action) -> impl FnOnce() + use<> {
        let (send, receive) = mpsc::channel();
        self.pending = Some(Pending {
            generation: self.generation,
            cancel: Arc::new(AtomicBool::new(false)),
            receive,
        });
        self.action = Some(action);
        move || {
            let _ = send.send(Ok(match action {
                Action::Load => Reply::Models(vec!["stale-model".into()]),
                Action::Test => Reply::Tested("stale success".into()),
            }));
        }
    }
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
    pub fn start(
        &mut self,
        action: Action,
        config: &Config,
        key: &str,
        cx: &mut gpui::App,
    ) -> Result<(), String> {
        if crate::shutdown::requested(cx) {
            return Err("Bello Box is closing. Provider setup requests are unavailable.".into());
        }
        if self.closed {
            return Err("Settings window is closed.".into());
        }
        if self.busy() {
            return Err("The previous request is still stopping. Retry when it finishes.".into());
        }
        // Validate before acquiring a worker or reading the network.
        enum Work {
            Load(bellobox_core::ai::provider_setup::ModelsRequest),
            Test(bellobox_core::ai::Request),
        }
        let work = match action {
            Action::Load => Work::Load(models_request(config.provider, &config.endpoint, key)?),
            Action::Test => Work::Test(test_request(config, key)?),
        };
        let lease = WorkerLease::acquire()?;
        // Acquired synchronously on the UI thread, but retired by the physical
        // worker. Closing/cancelling the Settings receiver cannot permit Quit.
        let quit_blocker =
            crate::shutdown::block_quit(cx, "Wait for the provider setup request to finish.");
        self.generation = self.generation.wrapping_add(1);
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let (send, receive) = mpsc::sync_channel(1);
        spawn_worker(
            move || {
                let lease = lease;
                let result = match work {
                    Work::Load(request) => {
                        crate::transport::provider_setup::load_models(request, worker_cancel)
                            .map(Reply::Models)
                    }
                    Work::Test(request) => {
                        crate::transport::provider_setup::test_connection(request, worker_cancel)
                            .map(Reply::Tested)
                    }
                };
                // Transport is fully retired before another action can start.
                drop(lease);
                drop(quit_blocker);
                let _ = send.send(result);
            },
            #[cfg(test)]
            std::mem::take(&mut self.fail_next_spawn),
        )
        .map_err(|_| "Cannot start provider setup worker.".to_owned())?;
        self.pending = Some(Pending {
            generation: self.generation,
            cancel,
            receive,
        });
        self.action = Some(action);
        match action {
            Action::Load => self.load_message = None,
            Action::Test => self.test_message = None,
        }
        Ok(())
    }
    /// Returns true only when a physical worker completion was observed.
    pub fn poll(&mut self) -> bool {
        let Some(pending) = &self.pending else {
            return false;
        };
        let result = match pending.receive.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return false,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("Provider setup worker stopped unexpectedly.".into())
            }
        };
        let current =
            pending.generation == self.generation && !pending.cancel.load(Ordering::Acquire);
        self.pending = None;
        if current {
            match result {
                Ok(Reply::Models(models)) => {
                    self.load_message = Some(if models.is_empty() {
                        "No models returned. Enter a model manually or use a preset.".into()
                    } else {
                        format!("Loaded {} models. Choose a model.", models.len())
                    });
                    self.models = models;
                }
                Ok(Reply::Tested(reply)) => self.test_message = Some(format!("Connected: {reply}")),
                Err(error) => match self.action {
                    Some(Action::Load) => {
                        self.models.clear();
                        self.load_message =
                            Some(format!("{error} Enter a model manually or use a preset."));
                    }
                    _ => self.test_message = Some(error),
                },
            }
        }
        self.action = None;
        true
    }
}
fn spawn_worker(
    work: impl FnOnce() + Send + 'static,
    #[cfg(test)] fail: bool,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    #[cfg(test)]
    if fail {
        // Exercise the same closure/guard disposal as a failed OS spawn.
        drop(work);
        return Err(std::io::Error::other("synthetic spawn failure"));
    }
    std::thread::Builder::new()
        .name("provider-setup".into())
        .spawn(work)
}

impl Drop for Setup {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pending(setup: &mut Setup, action: Action) -> mpsc::Sender<Result<Reply, String>> {
        let (send, receive) = mpsc::channel();
        setup.pending = Some(Pending {
            generation: setup.generation,
            cancel: Arc::new(AtomicBool::new(false)),
            receive,
        });
        setup.action = Some(action);
        send
    }
    #[test]
    fn editing_cancels_but_does_not_release_physical_ownership_or_accept_late_success() {
        let mut setup = Setup::default();
        let send = pending(&mut setup, Action::Test);
        let cancel = setup.pending.as_ref().unwrap().cancel.clone();
        setup.invalidate(false);
        assert!(cancel.load(Ordering::Acquire));
        assert!(setup.busy());
        send.send(Ok(Reply::Tested("old model".into()))).unwrap();
        assert!(setup.poll());
        assert!(!setup.busy());
        assert!(setup.test_message.is_none());
    }
    #[test]
    fn provider_endpoint_api_and_model_changes_reject_old_model_lists() {
        for clear_models in [true, false] {
            let mut setup = Setup::default();
            let send = pending(&mut setup, Action::Load);
            setup.invalidate(clear_models);
            send.send(Ok(Reply::Models(vec!["stale".into()]))).unwrap();
            setup.poll();
            assert!(setup.models.is_empty());
            assert!(setup.load_message.is_none());
        }
    }
    #[test]
    fn close_cancels_and_reopen_cannot_receive_old_result() {
        let mut old = Setup::default();
        let send = pending(&mut old, Action::Test);
        let cancel = old.pending.as_ref().unwrap().cancel.clone();
        drop(old);
        assert!(cancel.load(Ordering::Acquire));
        assert!(send.send(Ok(Reply::Tested("stale".into()))).is_err());
        let fresh = Setup::default();
        assert!(fresh.test_message.is_none() && fresh.models.is_empty());
    }
    #[test]
    fn empty_error_and_retry_leave_manual_model_available() {
        let mut setup = Setup::default();
        let send = pending(&mut setup, Action::Load);
        send.send(Ok(Reply::Models(vec![]))).unwrap();
        setup.poll();
        assert!(setup.load_message.as_ref().unwrap().contains("manually"));
        let send = pending(&mut setup, Action::Load);
        send.send(Err("HTTP 500".into())).unwrap();
        setup.poll();
        assert!(setup.load_message.as_ref().unwrap().contains("HTTP 500"));
        let send = pending(&mut setup, Action::Load);
        send.send(Ok(Reply::Models(vec!["chosen".into()]))).unwrap();
        setup.poll();
        assert_eq!(setup.models, ["chosen"]);
        setup.invalidate(false);
        assert_eq!(setup.models, ["chosen"]);
        setup.invalidate(true);
        assert!(setup.models.is_empty());
    }
    #[gpui::test]
    fn closed_view_keeps_physical_lane_until_held_request_retires(cx: &mut gpui::TestAppContext) {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            time::{Duration, Instant},
        };
        let _serial = TEST_LOCK.lock().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let config = Config {
            provider: bellobox_core::ai::Provider::OpenAIChat,
            endpoint: format!("http://{}/v1", listener.local_addr().unwrap()),
            model: "fixture".into(),
            system_prompt: String::new(),
            max_output_tokens: 20,
        };
        let (admitted, admission) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut head = Vec::new();
            while !head.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                assert_eq!(socket.read(&mut byte).unwrap(), 1);
                head.push(byte[0]);
                assert!(head.len() < 8192);
            }
            admitted.send(()).unwrap();
            released.recv_timeout(Duration::from_secs(5)).unwrap();
            let _ = socket.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{\"data\":[]}",
            );
        });
        let mut old = Setup::default();
        cx.update(|cx| old.start(Action::Load, &config, "fixture-only-key", cx))
            .unwrap();
        admission.recv_timeout(Duration::from_secs(5)).unwrap();
        old.invalidate(true);
        assert!(
            cx.update(|cx| old.start(Action::Load, &config, "fixture-only-key", cx))
                .is_err()
        );
        drop(old);
        let mut reopened = Setup::default();
        assert!(
            cx.update(|cx| reopened.start(Action::Load, &config, "fixture-only-key", cx))
                .unwrap_err()
                .contains("still stopping")
        );
        release.send(()).unwrap();
        worker.join().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while WORKER_BUSY.load(Ordering::Acquire) {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(reopened.models.is_empty() && reopened.test_message.is_none());
        let lease = WorkerLease::acquire().unwrap();
        drop(lease);
    }
}

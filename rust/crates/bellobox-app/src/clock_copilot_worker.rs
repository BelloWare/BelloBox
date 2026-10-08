//! A single physical HTTP lane. Cancellation invalidates presentation immediately,
//! but neither the lane nor app-owned Quit admission retires with a UI receiver.
use bellobox_core::{
    ai::Request,
    clock::copilot::protocol::{self, Reply},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

#[cfg(test)]
pub(crate) static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

static BUSY: AtomicBool = AtomicBool::new(false);
struct Lease;
impl Lease {
    fn acquire() -> Result<Self, String> {
        BUSY.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Self)
            .map_err(|_| {
                "The previous Copilot request is still stopping. Try again when it finishes.".into()
            })
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}
/// Observer only: cloning or dropping this never releases physical admission.
#[derive(Clone, Debug)]
pub(crate) struct RetirementGuard {
    cancel: Arc<AtomicBool>,
    retired: Arc<AtomicBool>,
}
impl RetirementGuard {
    pub fn same_request(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.retired, &other.retired)
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }
    pub fn physically_active(&self) -> bool {
        !self.retired.load(Ordering::Acquire)
    }
}
struct Retirement(Arc<AtomicBool>);
impl Drop for Retirement {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
struct Pending {
    generation: u64,
    cancel: Arc<AtomicBool>,
    retired: Arc<AtomicBool>,
    receive: mpsc::Receiver<Result<Reply, String>>,
}
#[derive(Default)]
pub(crate) struct Worker {
    pending: Option<Pending>,
}
impl Worker {
    pub fn guard(&self) -> Option<RetirementGuard> {
        self.pending.as_ref().map(|p| RetirementGuard {
            cancel: p.cancel.clone(),
            retired: p.retired.clone(),
        })
    }
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
    pub fn physically_active(&self) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|p| !p.retired.load(Ordering::Acquire))
    }
    pub fn cancel(&self) {
        if let Some(pending) = &self.pending {
            pending.cancel.store(true, Ordering::Release);
        }
    }
    pub fn start(
        &mut self,
        generation: u64,
        request: Request,
        blocker: crate::shutdown::QuitBlocker,
    ) -> Result<(), String> {
        if self.busy() {
            return Err(
                "The previous request is still stopping. Try again when it finishes.".into(),
            );
        }
        let lease = Lease::acquire()?;
        let cancel = Arc::new(AtomicBool::new(false));
        let retired = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let worker_retired = retired.clone();
        let (send, receive) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("world-clock-copilot".into())
            .spawn(move || {
                // Declared first, so panic also retires HTTP, blocker, lane before
                // the UI can observe the physical-retirement flag.
                let retirement = Retirement(worker_retired);
                let blocker = blocker;
                let lease = lease;
                let result = complete(request, worker_cancel.clone());
                let result = if worker_cancel.load(Ordering::Acquire) && result.is_ok() {
                    drop(result);
                    Err("Request cancelled.".into())
                } else {
                    result
                };
                drop(lease);
                drop(blocker);
                drop(retirement);
                let _ = send.send(result);
            })
            .map_err(|_| "Cannot start the Copilot worker.".to_owned())?;
        self.pending = Some(Pending {
            generation,
            cancel,
            retired,
            receive,
        });
        Ok(())
    }
    pub fn poll(&mut self) -> Option<(u64, Result<Reply, String>)> {
        let pending = self.pending.as_ref()?;
        let result = match pending.receive.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("Copilot worker stopped unexpectedly. Try again.".into())
            }
        };
        let generation = pending.generation;
        self.pending = None;
        Some((generation, result))
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn complete(request: Request, cancel: Arc<AtomicBool>) -> Result<Reply, String> {
    let mut output = String::new();
    let mut oversized = false;
    let result = crate::transport::send_cancellable(
        request,
        |text| {
            if output.len().saturating_add(text.len()) > 65_536 {
                oversized = true;
                cancel.store(true, Ordering::Release);
            } else if !oversized {
                output.push_str(text);
            }
        },
        cancel.clone(),
    );
    if oversized {
        return Err("Copilot response exceeds the 64 KiB limit.".into());
    }
    result?;
    if cancel.load(Ordering::Acquire) {
        return Err("Request cancelled.".into());
    }
    protocol::parse_response(&output)
}

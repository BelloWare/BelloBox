//! Physical callback ownership, independent of logical cancellation/deadlines.
use std::sync::{Arc, Condvar, Mutex};
#[derive(Default)]
pub(super) struct Callbacks(Arc<(Mutex<usize>, Condvar)>);
pub(super) struct Ticket(Arc<(Mutex<usize>, Condvar)>);
impl Callbacks {
    pub fn ticket(&self) -> Ticket {
        *self.0 .0.lock().unwrap_or_else(|e| e.into_inner()) += 1;
        Ticket(self.0.clone())
    }
    pub fn wait(&self) {
        let (count, wake) = &*self.0;
        let mut count = count.lock().unwrap_or_else(|e| e.into_inner());
        while *count != 0 {
            count = wake.wait(count).unwrap_or_else(|e| e.into_inner());
        }
    }
}
impl Drop for Ticket {
    fn drop(&mut self) {
        let (count, wake) = &*self.0;
        *count.lock().unwrap_or_else(|e| e.into_inner()) -= 1;
        wake.notify_all();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn logical_cancellation_cannot_complete_held_callback_retirement() {
        let callbacks = Arc::new(Callbacks::default());
        let ticket = callbacks.ticket();
        let (started, start) = std::sync::mpsc::channel();
        let (done, ended) = std::sync::mpsc::channel();
        let owner = callbacks.clone();
        let worker = std::thread::spawn(move || {
            started.send(()).unwrap();
            owner.wait();
            done.send(()).unwrap();
        });
        start.recv().unwrap();
        assert!(ended.try_recv().is_err());
        drop(ticket);
        ended
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        worker.join().unwrap();
    }
}

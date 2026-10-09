//! App-controlled quit waits for physical media retirement before asking GPUI to
//! quit. GPUI's 100 ms shutdown observer budget is not a native drain deadline.
//! The app-owned Quit action and shortcut use this gate. Native platform
//! termination (including Dock Quit), forced quit and OS shutdown can bypass it.
//! Recovery remains best effort for those paths.
use gpui::{App, AppContext, BorrowAppContext, Global, KeyBinding, Window};
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll, Waker},
};

mod quit;
pub(crate) use quit::{
    Admission as QuitAdmission, Blocker as QuitBlocker, admit_window as admit_quit_window,
    block as block_quit, guard as guard_quit, is_feedback_window as is_quit_feedback_window,
};

#[cfg(test)]
pub(crate) use quit::{dismiss_refusal, pending_refusal};

gpui::actions!(bellobox, [Quit]);

#[cfg(target_os = "macos")]
const QUIT_KEYSTROKE: &str = "cmd-q";
#[cfg(not(target_os = "macos"))]
const QUIT_KEYSTROKE: &str = "ctrl-q";

type Cancel = Arc<dyn Fn() + Send + Sync>;
#[derive(Default)]
struct State {
    requested: bool,
    next: u64,
    active: HashMap<u64, Cancel>,
    waiter: Option<Waker>,
}
#[derive(Clone, Default)]
pub(crate) struct Registry(Arc<Mutex<State>>);
impl Registry {
    /// Admit before starting work. The cancellation closure must only signal
    /// atomics, never wait for a worker, filesystem operation or native callback.
    pub fn admit(&self, cancel: impl Fn() + Send + Sync + 'static) -> Option<Ticket> {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.requested {
            drop(state);
            cancel();
            return None;
        }
        state.next += 1;
        let id = state.next;
        state.active.insert(id, Arc::new(cancel));
        Some(Ticket {
            registry: self.clone(),
            id,
        })
    }
    fn request(&self) -> bool {
        let cancellations = {
            let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
            if state.requested {
                return false;
            }
            state.requested = true;
            state.active.values().cloned().collect::<Vec<_>>()
        };
        // Do not hold the registry lock while calling even an atomic control.
        for cancel in cancellations {
            cancel();
        }
        true
    }
    pub fn requested(&self) -> bool {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).requested
    }
    fn drained(self) -> Drain {
        Drain(self)
    }
    pub fn active(&self) -> usize {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .active
            .len()
    }
}
/// Owned by the physical worker, never its UI completion future. Drop only after
/// native callbacks/pools, staged output and expensive worker locals are retired.
pub(crate) struct Ticket {
    registry: Registry,
    id: u64,
}
impl Drop for Ticket {
    fn drop(&mut self) {
        let (cancel, wake) = {
            let mut state = self.registry.0.lock().unwrap_or_else(|e| e.into_inner());
            let cancel = state.active.remove(&self.id);
            let wake = if state.active.is_empty() {
                state.waiter.take()
            } else {
                None
            };
            (cancel, wake)
        };
        drop(cancel);
        if let Some(wake) = wake {
            wake.wake();
        }
    }
}
struct Drain(Registry);
impl Future for Drain {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let mut state = self.0.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.active.is_empty() {
            Poll::Ready(())
        } else {
            state.waiter = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}
// One bounded terminal slot, with physical ownership retained by its worker.
// Dropping a GPUI foreground receiver does not drop native/file payloads there.
struct CompletionState<T> {
    result: Option<T>,
    transferred: bool,
    retired: bool,
    aborted: bool,
    waiter: Option<Waker>,
    retirement_waiter: Option<Waker>,
}
struct CompletionShared<T> {
    state: Mutex<CompletionState<T>>,
    closed: AtomicBool,
}
impl<T> CompletionShared<T> {
    fn close(&self) {
        self.closed.store(true, Ordering::Release);
        let wake = self
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retirement_waiter
            .take();
        if let Some(wake) = wake {
            wake.wake();
        }
    }
    fn finish(&self, result: T) {
        let wake = {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            state.result = Some(result);
            state.waiter.take()
        };
        if let Some(wake) = wake {
            wake.wake();
        }
    }
}
/// Ensures an unwinding work closure cannot leave readiness permanently pending.
/// Declared inside the physical worker, after its ticket and before work starts.
struct WorkerEnd<T>(Arc<CompletionShared<T>>);
impl<T> Drop for WorkerEnd<T> {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.retired {
            return;
        }
        self.0.closed.store(true, Ordering::Release);
        state.aborted = true;
        let rejected = state.result.take();
        drop(state);
        drop(rejected);
        let wake = {
            let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
            state.retired = true;
            state.waiter.take()
        };
        if let Some(wake) = wake {
            wake.wake();
        }
    }
}
struct Retire<T>(Arc<CompletionShared<T>>);
impl<T> Future for Retire<T> {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.transferred || self.0.closed.load(Ordering::Acquire) {
            let rejected = state.result.take();
            drop(state);
            // Only this background future disposes a rejected terminal owner.
            drop(rejected);
            let wake = {
                let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
                state.retired = true;
                state.waiter.take()
            };
            if let Some(wake) = wake {
                wake.wake();
            }
            Poll::Ready(())
        } else {
            state.retirement_waiter = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}
pub(crate) struct Completion<T>(Arc<CompletionShared<T>>);
struct Ready<'a, T>(&'a Completion<T>);
impl<T> Future for Ready<'_, T> {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let mut state = self.0.0.state.lock().unwrap_or_else(|e| e.into_inner());
        let ready = if self.0.0.closed.load(Ordering::Acquire) {
            state.retired
        } else {
            state.result.is_some()
        };
        if ready {
            Poll::Ready(())
        } else {
            state.waiter = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}
impl<T> Completion<T> {
    pub fn ready(&self) -> impl Future<Output = ()> + '_ {
        Ready(self)
    }
    /// Call only after the final current-owner check, immediately at adoption.
    /// Readiness alone never moves the payload off its physical worker.
    pub fn take(&mut self) -> Option<T> {
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        if self.0.closed.load(Ordering::Acquire) {
            return None;
        }
        let result = state.result.take()?;
        state.transferred = true;
        let wake = state.retirement_waiter.take();
        drop(state);
        if let Some(wake) = wake {
            wake.wake();
        }
        Some(result)
    }
    pub fn close(&self) {
        self.0.close();
    }
    pub fn aborted(&self) -> bool {
        self.0
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .aborted
    }
}
#[derive(Clone)]
pub(crate) struct Close(Arc<dyn Fn() + Send + Sync>);
impl Close {
    pub fn close(&self) {
        (self.0)();
    }
}
impl<T: Send + 'static> Completion<T> {
    pub fn close_handle(&self) -> Close {
        let shared = self.0.clone();
        Close(Arc::new(move || shared.close()))
    }
}
impl<T> Drop for Completion<T> {
    fn drop(&mut self) {
        self.0.close();
    }
}

/// Reserve before constructing/moving the owning work closure. A rejected
/// admission therefore never receives an owned reader/movie/request to dispose.
pub(crate) fn admit<T: Send + 'static>(
    cx: &mut App,
    cancel: impl Fn() + Send + Sync + 'static,
) -> Option<Permit<T>> {
    let shared = Arc::new(CompletionShared {
        state: Mutex::new(CompletionState {
            result: None,
            transferred: false,
            retired: false,
            aborted: false,
            waiter: None,
            retirement_waiter: None,
        }),
        closed: AtomicBool::new(false),
    });
    let close = shared.clone();
    let ticket = registry(cx).admit(move || {
        cancel();
        close.close();
    })?;
    Some(Permit { ticket, shared })
}
pub(crate) struct Permit<T> {
    ticket: Ticket,
    shared: Arc<CompletionShared<T>>,
}
impl<T: Send + 'static> Permit<T> {
    /// The detached physical job owns both the ticket and its bounded terminal
    /// slot. Only explicit current-owner adoption transfers an accepted result.
    pub fn spawn(self, cx: &mut App, work: impl FnOnce() -> T + Send + 'static) -> Completion<T> {
        let Self { ticket, shared } = self;
        let receiver = Completion(shared.clone());
        #[cfg(test)]
        let inject_panic = std::mem::take(&mut cx.global_mut::<Shutdown>().panic_next);
        cx.background_executor()
            .spawn(async move {
                let _ticket = ticket;
                let _end = WorkerEnd(shared.clone());
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    #[cfg(test)]
                    if inject_panic {
                        panic!("injected media worker panic");
                    }
                    work()
                }));
                if let Ok(result) = result {
                    shared.finish(result);
                    Retire(shared).await;
                }
                // WorkerEnd turns an unwind into a closed, aborted terminal
                // state. Callers report failure only for a still-current owner.
                // Rejected payload disposal precedes ticket drop and quit wake.
            })
            .detach();
        receiver
    }
}

#[derive(Default)]
pub(crate) struct Shutdown {
    registry: Registry,
    #[cfg(test)]
    quit_calls: usize,
    #[cfg(test)]
    panic_next: bool,
}
impl Global for Shutdown {}

pub(crate) fn registry(cx: &mut App) -> Registry {
    if !cx.has_global::<Shutdown>() {
        cx.set_global(Shutdown::default());
    }
    cx.global::<Shutdown>().registry.clone()
}
pub(crate) fn requested(cx: &App) -> bool {
    cx.try_global::<Shutdown>()
        .is_some_and(|state| state.registry.requested())
}
/// The only App-controlled exit. All windows/entities may remain retained while
/// the app executor stays alive to service retirement. There is no timeout.
pub(crate) fn request_quit(cx: &mut App) {
    let registry = registry(cx);
    if !registry.request() {
        return;
    }
    // Close/invalidate every retained recording and converter entity immediately
    // through its global observer, independently of window or entity destruction.
    cx.update_global::<Shutdown, _>(|_, _| ());
    // Global actions run while their active window is borrowed. Do not update
    // any existing window until dispatch returns it to App. Retaining these
    // native windows keeps Linux's event loop alive throughout physical drain.
    cx.defer(|cx| {
        for handle in cx.windows() {
            let _ = cx.update_window(handle, |_, window, cx| show_closing(window, cx));
        }
    });
    cx.spawn(async move |cx| {
        registry.drained().await;
        let _ = cx.update(|cx| {
            #[cfg(test)]
            {
                cx.global_mut::<Shutdown>().quit_calls += 1;
            }
            cx.quit();
        });
    })
    .detach();
}
/// Register on ordinary root windows; roots with their own close policy call
/// allow_close after their existing cancellation/discard decision instead.
pub(crate) fn guard_window(window: &mut Window, cx: &mut App) {
    admit_quit_window(window, cx);
    window.on_window_should_close(cx, allow_close);
}
/// Keep the final existing native window alive. GPUI's X11/Wayland backends stop
/// their event loop at zero windows before on_app_quit can wait for anything.
/// Never create a replacement window from a native should-close callback: X11
/// still holds its client borrow there. Replacing this window's root is local.
pub(crate) fn allow_close(window: &mut Window, cx: &mut App) -> bool {
    let pending = cx
        .try_global::<Shutdown>()
        .is_some_and(|s| s.registry.active() > 0);
    if requested(cx) || (cx.windows().len() <= 1 && pending) {
        request_quit(cx);
        show_closing(window, cx);
        return false;
    }
    true
}
/// All application-owned programmatic closes use the same veto as titlebar
/// close. Direct GPUI remove_window is reserved for this admitted branch/tests.
pub(crate) fn close_window(window: &mut Window, cx: &mut App) {
    if allow_close(window, cx) {
        window.remove_window();
    }
}
fn show_closing(window: &mut Window, cx: &mut App) {
    if window.root::<Closing>().flatten().is_none() {
        window.replace_root(cx, |_, _| Closing);
        window.set_window_title("Closing — Bello Box");
    }
}
struct Closing;
impl gpui::Render for Closing {
    fn render(
        &mut self,
        window: &mut Window,
        _: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        use gpui::{prelude::*, *};
        let palette = crate::theme::for_window(window);
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .bg(palette.bg)
            .text_color(palette.primary)
            .font_family(crate::theme::ui_font())
            .child("Closing…")
            .child(
                div()
                    .text_sm()
                    .text_color(palette.secondary)
                    .child("Waiting for active work and cleanup to finish."),
            )
    }
}
#[cfg(test)]
pub(crate) fn is_closing(window: &Window) -> bool {
    window.root::<Closing>().flatten().is_some()
}

/// Called once by the desktop host. Test hosts use the exact same exit route.
pub(crate) fn init(cx: &mut App) {
    // Add only our shortcut; preserve all existing bindings and native menus.
    // This does not intercept NSApplication's Dock/OS termination callback.
    cx.bind_keys([KeyBinding::new(QUIT_KEYSTROKE, Quit, None)]);
    cx.on_action(|_: &Quit, cx| quit::request(cx));
    let registry = registry(cx);
    cx.on_app_quit(move |_| {
        // This observer cannot veto native platform termination, including
        // Dock Quit. Signal promptly, but do not call an immediately-ready
        // future a physical retirement guarantee.
        registry.request();
        async {}
    })
    .detach();
    cx.on_window_closed(|cx| {
        if cx.windows().is_empty() {
            // Native close/input callbacks still borrow X11. A foreground task
            // escapes that callback and permits a replacement window to appear.
            cx.spawn(async move |cx| {
                let _ = cx.update(|cx| {
                    if cx.windows().is_empty() {
                        request_quit(cx);
                    }
                });
            })
            .detach();
        }
    })
    .detach();
}
#[cfg(test)]
pub(crate) fn quit_calls(cx: &App) -> usize {
    cx.global::<Shutdown>().quit_calls
}
#[cfg(test)]
mod tests;

#[cfg(all(test, feature = "recording-fixtures"))]
pub(crate) fn panic_next(cx: &mut App) {
    cx.global_mut::<Shutdown>().panic_next = true;
}

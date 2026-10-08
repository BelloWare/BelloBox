//! Admission for the app-owned Quit action. Refusal abandons the attempt: no
//! discard approval or pending continuation is retained for a later action.
use gpui::{
    AnyWindowHandle, App, AppContext, Context, EntityId, Global, WeakEntity, Window, WindowHandle,
    WindowId, WindowOptions, div, prelude::*,
};
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{Arc, Mutex},
};

#[derive(Clone, Copy)]
pub(crate) enum Admission {
    Ready,
    /// The owner has shown its existing local prompt/status.
    Refused,
    Explain(&'static str),
}
mod prompt;
#[cfg(test)]
pub(crate) use prompt::{dismiss_with_input as dismiss_refusal, pending as pending_refusal};

type Check = Rc<dyn Fn(&mut App) -> Admission>;
#[derive(Default)]
struct Work {
    next: u64,
    active: HashMap<u64, &'static str>,
}
#[derive(Default)]
struct State {
    windows: HashSet<WindowId>,
    checks: HashMap<EntityId, (WindowId, Check)>,
    work: Arc<Mutex<Work>>,
    queued: bool,
    explaining: bool,
    active_prompt: Option<(AnyWindowHandle, WeakEntity<prompt::QuitPrompt>)>,
    notice: Option<WindowHandle<Notice>>,
    observing_close: bool,
    #[cfg(test)]
    panic_prompt_construction: bool,
}
impl Global for State {}

pub(crate) fn admit_window(window: &Window, cx: &mut App) {
    if !cx.default_global::<State>().observing_close {
        cx.default_global::<State>().observing_close = true;
        cx.on_window_closed(|cx| {
            let live: HashSet<_> = cx
                .windows()
                .iter()
                .map(|window| window.window_id())
                .collect();
            let state = cx.default_global::<State>();
            state.windows.retain(|id| live.contains(id));
            state.checks.retain(|_, (id, _)| live.contains(id));
            if state
                .notice
                .is_some_and(|notice| !live.contains(&notice.window_id()))
            {
                state.notice = None;
            }
        })
        .detach();
    }
    cx.default_global::<State>()
        .windows
        .insert(window.window_handle().window_id());
}

/// The callback is invoked only after action dispatch releases its window.
/// Closed windows and retired entities cannot veto a later Quit attempt.
pub(crate) fn guard<T: 'static>(
    window: &Window,
    cx: &mut Context<T>,
    check: impl Fn(&mut T, &mut Window, &mut Context<T>) -> Admission + 'static,
) {
    admit_window(window, cx);
    let handle = window.window_handle();
    let weak = cx.weak_entity();
    let id = cx.entity_id();
    cx.default_global::<State>().checks.insert(
        id,
        (
            handle.window_id(),
            Rc::new(move |cx| {
                if !cx.windows().contains(&handle) {
                    return Admission::Ready;
                }
                let Some(owner) = weak.upgrade() else {
                    return Admission::Ready;
                };
                handle
                    .update(cx, |_, window, cx| {
                        owner.update(cx, |owner, cx| check(owner, window, cx))
                    })
                    .unwrap_or(Admission::Explain("Finish this window's current action."))
            }),
        ),
    );
    cx.on_release(move |_, cx| {
        cx.default_global::<State>().checks.remove(&id);
    })
    .detach();
}

/// Physical owner for work outside the media-drain contract. Move this guard
/// with the actual worker, not its UI receiver, and never cancel its publication.
struct BlockerOwner {
    work: Arc<Mutex<Work>>,
    id: u64,
}
impl Drop for BlockerOwner {
    fn drop(&mut self) {
        self.work
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .active
            .remove(&self.id);
    }
}
#[derive(Clone)]
pub(crate) struct Blocker {
    _owner: Arc<BlockerOwner>,
}
pub(crate) fn block(cx: &mut App, reason: &'static str) -> Blocker {
    let work = cx.default_global::<State>().work.clone();
    let id = {
        let mut state = work.lock().unwrap_or_else(|e| e.into_inner());
        state.next += 1;
        let id = state.next;
        state.active.insert(id, reason);
        id
    };
    Blocker {
        _owner: Arc::new(BlockerOwner { work, id }),
    }
}
fn blocked_work(cx: &App) -> Option<&'static str> {
    cx.try_global::<State>().and_then(|state| {
        state
            .work
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .active
            .values()
            .next()
            .copied()
    })
}

pub(super) fn request(cx: &mut App) {
    if super::requested(cx) || cx.default_global::<State>().queued {
        return;
    }
    cx.default_global::<State>().queued = true;
    cx.defer(|cx| {
        cx.default_global::<State>().queued = false;
        if super::requested(cx) {
            return;
        }
        // A foreground pointer owner must be allowed to refuse in its own
        // chrome before a different guard/worker can overlay that gesture.
        // Collect all guards for that window: an inline editor and its selector
        // can share it. Refused never authorizes a quit or waives later checks.
        let feedback = feedback_window(cx).map(|window| window.window_id());
        let checks = cx
            .default_global::<State>()
            .checks
            .values()
            .map(|(id, check)| (*id, check.clone()))
            .collect::<Vec<_>>();
        let mut locally_refused = false;
        let mut explanation = None;
        for (_, check) in checks.iter().filter(|(id, _)| Some(*id) == feedback) {
            match check(cx) {
                Admission::Ready => {}
                Admission::Refused => locally_refused = true,
                Admission::Explain(reason) => {
                    explanation.get_or_insert(reason);
                }
            }
        }
        if locally_refused {
            return;
        }
        if let Some(reason) = explanation.or_else(|| blocked_work(cx)) {
            explain(reason, cx);
            return;
        }
        for (_, check) in checks.iter().filter(|(id, _)| Some(*id) != feedback) {
            match check(cx) {
                Admission::Ready => {}
                Admission::Refused => return,
                Admission::Explain(reason) => {
                    explain(reason, cx);
                    return;
                }
            }
        }
        let windows = cx.windows();
        let admitted = &mut cx.default_global::<State>().windows;
        admitted.retain(|id| windows.iter().any(|window| window.window_id() == *id));
        if windows
            .iter()
            .any(|window| !admitted.contains(&window.window_id()))
        {
            explain("Close the capture or tool window first.", cx);
            return;
        }
        // Checks and latch share one foreground update. No event, edit or
        // asynchronous approval can intervene or authorize a later attempt.
        super::request_quit(cx);
    });
}

fn feedback_window(cx: &App) -> Option<AnyWindowHandle> {
    cx.active_window().or_else(|| cx.windows().first().copied())
}
pub(crate) fn is_feedback_window(window: &Window, cx: &App) -> bool {
    feedback_window(cx) == Some(window.window_handle())
}

fn explain(reason: &'static str, cx: &mut App) {
    if let Some(notice) = cx.default_global::<State>().notice
        && cx.windows().contains(&notice.into())
    {
        let _ = notice.update(cx, |notice, _, cx| {
            notice.0 = reason;
            cx.notify();
        });
        return;
    }

    if cx.default_global::<State>().explaining {
        return;
    }
    let handle = feedback_window(cx);
    if let Some(handle) = handle {
        cx.default_global::<State>().explaining = true;
        let result = handle.update(cx, |_, window, cx| prompt::show(reason, window, cx));
        if let Ok(answer) = result {
            cx.spawn(async move |cx| {
                answer.await;
                let _ = cx.update(|cx| {
                    let state = cx.default_global::<State>();
                    state.explaining = false;
                    state.active_prompt = None;
                });
            })
            .detach();
        } else {
            let state = cx.default_global::<State>();
            state.explaining = false;
            state.active_prompt = None;
        }
    } else {
        // macOS can dispatch the app action with no native window. Keep its
        // refusal visible rather than silently dropping the user's request.
        let notice = cx.open_window(WindowOptions::default(), |window, cx| {
            admit_window(window, cx);
            window.set_window_title("Bello Box is still open");
            window.on_window_should_close(cx, |window, cx| {
                blocked_work(cx).is_none() && super::allow_close(window, cx)
            });
            cx.new(|_| Notice(reason))
        });
        cx.default_global::<State>().notice = notice.ok();
    }
}
struct Notice(&'static str);
impl Render for Notice {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let palette = crate::theme::for_window(window);
        div()
            .size_full()
            .bg(palette.bg)
            .text_color(palette.primary)
            .font_family(crate::theme::ui_font())
            .p_6()
            .flex()
            .flex_col()
            .gap_4()
            .child("Bello Box is still open")
            .child(self.0)
            .child("When this work finishes, close this notice or choose Quit again.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{TestAppContext, VisualTestContext};
    struct Owner;
    impl Render for Owner {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }
    #[gpui::test]
    fn live_borrowed_owner_update_failure_refuses_instead_of_waiving_its_guard(
        cx: &mut TestAppContext,
    ) {
        let window = cx.add_window(|window, cx| {
            guard(window, cx, |_, _, _| Admission::Refused);
            Owner
        });
        window
            .update(cx, |_, _, cx| {
                let check = cx
                    .global::<State>()
                    .checks
                    .values()
                    .next()
                    .unwrap()
                    .1
                    .clone();
                assert!(matches!(check(cx), Admission::Explain(_)));
            })
            .unwrap();
    }
    #[gpui::test]
    fn no_window_refusal_notice_is_single_and_can_close_after_physical_work(
        cx: &mut TestAppContext,
    ) {
        cx.update(super::super::init);
        let blocker = cx.update(|cx| block(cx, "A QR image is still being saved."));
        for _ in 0..3 {
            cx.update(|cx| cx.dispatch_action(&super::super::Quit));
            cx.run_until_parked();
            assert_eq!(cx.read(|cx| cx.windows().len()), 1);
            assert!(!cx.has_pending_prompt());
            assert!(!cx.read(super::super::requested));
        }
        let notice = cx.update(|cx| cx.global::<State>().notice.unwrap());
        let mut visual = VisualTestContext::from_window(notice.into(), cx);
        assert!(!visual.simulate_close());
        drop(blocker);
        assert!(visual.simulate_close());
        cx.update(|cx| cx.dispatch_action(&super::super::Quit));
        cx.run_until_parked();
        assert_eq!(cx.read(super::super::quit_calls), 1);
    }
}

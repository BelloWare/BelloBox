//! A Quit-only modal input barrier. Pinned GPUI's stock fallback prompt paints
//! above the root without occluding it or consuming its dismissal click.
use std::collections::HashSet;

use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    KeyDownEvent, KeyUpEvent, PromptHandle, PromptResponse, Render, Window, div, prelude::*, px,
};

pub(super) struct QuitPrompt {
    reason: String,
    focus: FocusHandle,
    armed_key: Option<String>,
    dismissed: bool,
    pressed_keys: HashSet<String>,
    dismiss_requested: bool,
}
impl EventEmitter<PromptResponse> for QuitPrompt {}
impl Focusable for QuitPrompt {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl QuitPrompt {
    fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.prevent_default();
        cx.stop_propagation();
        self.dismiss_requested = true;
        self.finish_dismissal(window, cx);
    }
    fn finish_dismissal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dismiss_requested
            && self.pressed_keys.is_empty()
            && !window.modifiers().modified()
            && !self.dismissed
        {
            self.dismissed = true;
            // GPUI only refreshes through restored focus when one existed.
            // A root without prior focus must also retire the modal frame.
            window.refresh();
            cx.emit(PromptResponse(0));
        } else {
            cx.notify();
        }
    }
    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.prevent_default();
        cx.stop_propagation();
        self.pressed_keys.insert(event.keystroke.key.to_string());
        if !event.is_held {
            self.armed_key = (!event.keystroke.modifiers.modified()
                && matches!(event.keystroke.key.as_str(), "enter" | "space" | "escape"))
            .then(|| event.keystroke.key.to_string());
        }
    }
    fn key_up(&mut self, event: &KeyUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.prevent_default();
        cx.stop_propagation();
        self.pressed_keys.remove(event.keystroke.key.as_str());
        // Keep focus on the modal until the complete key gesture is consumed;
        // dismissal on key-down would deliver its key-up to the restored root.
        if self.armed_key.take().as_deref() == Some(event.keystroke.key.as_str())
            && !event.keystroke.modifiers.modified()
        {
            self.dismiss_requested = true;
        }
        self.finish_dismissal(window, cx);
    }
}
impl Render for QuitPrompt {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = crate::theme::for_window(window);
        let mut barrier = div()
            .id("quit-refusal-barrier")
            .size_full()
            .occlude()
            .track_focus(&self.focus)
            .on_any_mouse_down(|_, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
            })
            .on_mouse_move(|_, _, cx| cx.stop_propagation())
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .capture_key_down(cx.listener(Self::key_down))
            .capture_key_up(cx.listener(Self::key_up))
            .on_modifiers_changed(cx.listener(|this, _, window, cx| {
                cx.stop_propagation();
                this.finish_dismissal(window, cx);
            }))
            .on_action(|_: &super::super::Quit, _, cx| cx.stop_propagation())
            .bg(gpui::black().opacity(0.35))
            .flex()
            .items_center()
            .justify_center()
            .p_4()
            .child(
                div()
                    .w(px(360.))
                    .max_w_full()
                    .p_4()
                    .rounded_lg()
                    .bg(palette.surface)
                    .text_color(palette.primary)
                    .font_family(crate::theme::ui_font())
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child("Bello Box is still open")
                    .child(
                        div()
                            .w_full()
                            .text_sm()
                            .whitespace_normal()
                            .child(self.reason.clone()),
                    )
                    .when(self.dismiss_requested, |card| {
                        card.child(div().text_sm().child("Release held keys to close."))
                    })
                    .child(
                        div()
                            .id("quit-refusal-ok")
                            .debug_selector(|| "quit-refusal-ok".into())
                            .w_full()
                            .py_2()
                            .text_center()
                            .rounded_md()
                            .bg(palette.well)
                            .cursor_pointer()
                            .child("OK")
                            .on_click(cx.listener(|this, _, window, cx| this.dismiss(window, cx))),
                    ),
            );
        barrier.interactivity().on_any_mouse_up(|_, window, cx| {
            window.prevent_default();
            cx.stop_propagation();
        });
        barrier
    }
}

/// BelloBox uses GPUI's default prompt builder everywhere else. The override is
/// consumed synchronously by Window::prompt and reset before an event turn or
/// returned future. Even an unwinding construction restores that default.
pub(super) fn show(
    reason: &'static str,
    window: &mut Window,
    cx: &mut App,
) -> impl std::future::Future<Output = ()> + use<> {
    cx.set_prompt_builder(|_, _, detail, _, handle: PromptHandle, window, cx| {
        #[cfg(test)]
        if std::mem::take(
            &mut cx
                .default_global::<super::State>()
                .panic_prompt_construction,
        ) {
            panic!("injected Quit prompt construction failure");
        }
        let view: Entity<QuitPrompt> = cx.new(|cx| {
            cx.observe_window_activation(window, |this: &mut QuitPrompt, window, cx| {
                if !window.is_window_active() {
                    // Key releases may be delivered to another application.
                    // Abandon the partial gesture; the next deliberate OK/key
                    // gesture after returning can dismiss without a stale hold.
                    this.pressed_keys.clear();
                    this.armed_key = None;
                    this.dismiss_requested = false;
                    cx.notify();
                }
            })
            .detach();
            QuitPrompt {
                reason: detail.unwrap_or_default().to_owned(),
                focus: cx.focus_handle(),
                armed_key: None,
                dismissed: false,
                pressed_keys: HashSet::new(),
                dismiss_requested: false,
            }
        });
        cx.default_global::<super::State>().active_prompt =
            Some((window.window_handle(), view.downgrade()));
        handle.with_view(view, window, cx)
    });
    let answer = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        window.prompt(
            gpui::PromptLevel::Info,
            "Bello Box is still open",
            Some(reason),
            &["OK"],
            cx,
        )
    }));
    cx.reset_prompt_builder();
    let answer = match answer {
        Ok(answer) => answer,
        Err(payload) => {
            let state = cx.default_global::<super::State>();
            state.explaining = false;
            state.active_prompt = None;
            std::panic::resume_unwind(payload)
        }
    };
    async move {
        let _ = answer.await;
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) fn pending(cx: &gpui::TestAppContext) -> Option<(String, String)> {
    cx.read(|cx| {
        let (_, view) = cx.try_global::<super::State>()?.active_prompt.as_ref()?;
        let view = view.upgrade()?;
        Some((
            "Bello Box is still open".into(),
            view.read(cx).reason.clone(),
        ))
    })
}

#[cfg(test)]
pub(crate) fn dismiss_with_input(cx: &mut gpui::TestAppContext) {
    let window = cx.read(|cx| {
        cx.global::<super::State>()
            .active_prompt
            .as_ref()
            .expect("Quit refusal")
            .0
    });
    window
        .update(cx, |_, window, cx| window.draw(cx).clear())
        .unwrap();
    let mut visual = gpui::VisualTestContext::from_window(window, cx);
    visual.simulate_event(KeyDownEvent {
        keystroke: gpui::Keystroke::parse("enter").unwrap(),
        is_held: false,
    });
    visual.simulate_event(KeyUpEvent {
        keystroke: gpui::Keystroke::parse("enter").unwrap(),
    });
    cx.run_until_parked();
    window
        .update(cx, |_, window, cx| window.draw(cx).clear())
        .unwrap();
}

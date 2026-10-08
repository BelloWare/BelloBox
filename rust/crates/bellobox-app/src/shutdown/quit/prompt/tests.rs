use super::*;
use gpui::{
    AnyWindowHandle, InputEvent, KeyDownEvent, KeyUpEvent, Keystroke, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollDelta, ScrollWheelEvent,
    TestAppContext, VisualTestContext, WindowHandle, point,
};

struct Underlay {
    focus: FocusHandle,
    editor: Entity<bello_workbench_ui::EditorView>,
    text_mode: bool,
    downs: usize,
    ups: usize,
    clicks: usize,
    keys: usize,
    scrolls: usize,
}
impl Render for Underlay {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut underlay = div()
            .id("quit-test-underlay")
            .size_full()
            .track_focus(&self.focus)
            .on_any_mouse_down(cx.listener(|this, _, _, _| this.downs += 1))
            .on_click(cx.listener(|this, _, _, _| this.clicks += 1))
            .on_key_down(cx.listener(|this, _, _, _| this.keys += 1))
            .on_key_up(cx.listener(|this, _, _, _| this.keys += 1))
            .on_scroll_wheel(cx.listener(|this, _, _, _| this.scrolls += 1))
            .when(self.text_mode, |div| div.child(self.editor.clone()));
        underlay
            .interactivity()
            .on_any_mouse_up(cx.listener(|this, _, _, _| this.ups += 1));
        underlay
    }
}
fn setup(cx: &mut TestAppContext, text_mode: bool) -> WindowHandle<Underlay> {
    cx.update(crate::shutdown::init);
    let handle = cx.add_window(|window, cx| {
        crate::shutdown::guard_window(window, cx);
        let editor = cx.new(|cx| {
            let mut editor = bello_workbench_ui::EditorView::new("kept".into(), window, cx);
            editor.set_compact(true, cx);
            editor
        });
        let focus = cx.focus_handle();
        if text_mode {
            editor.read(cx).focus(window);
        } else {
            focus.focus(window);
        }
        Underlay {
            focus,
            editor,
            text_mode,
            downs: 0,
            ups: 0,
            clicks: 0,
            keys: 0,
            scrolls: 0,
        }
    });
    handle
        .update(cx, |_, window, _| window.activate_window())
        .unwrap();
    draw(handle.into(), cx);
    handle
}
fn draw(handle: AnyWindowHandle, cx: &mut TestAppContext) {
    handle
        .update(cx, |_, window, cx| window.draw(cx).clear())
        .unwrap();
}
fn explain(handle: AnyWindowHandle, cx: &mut TestAppContext) {
    cx.update(|cx| super::super::explain("Finish this tool's current action.", cx));
    cx.run_until_parked();
    draw(handle, cx);
    assert!(pending(cx).is_some());
}
fn send(handle: AnyWindowHandle, event: impl InputEvent, cx: &mut TestAppContext) {
    VisualTestContext::from_window(handle, cx).simulate_event(event);
    cx.run_until_parked();
}
fn down(handle: AnyWindowHandle, position: Point<Pixels>, cx: &mut TestAppContext) {
    send(
        handle,
        MouseMoveEvent {
            position,
            pressed_button: None,
            modifiers: Modifiers::default(),
        },
        cx,
    );
    send(
        handle,
        MouseDownEvent {
            position,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        },
        cx,
    );
}
fn up(handle: AnyWindowHandle, position: Point<Pixels>, cx: &mut TestAppContext) {
    send(
        handle,
        MouseUpEvent {
            position,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        },
        cx,
    );
}
fn ok_center(handle: AnyWindowHandle, cx: &mut TestAppContext) -> Point<Pixels> {
    VisualTestContext::from_window(handle, cx)
        .debug_bounds("quit-refusal-ok")
        .expect("real modal OK hitbox")
        .center()
}
fn assert_no_underlay_input(handle: WindowHandle<Underlay>, cx: &TestAppContext) {
    handle
        .read_with(cx, |view, cx| {
            assert_eq!(
                (view.downs, view.ups, view.clicks, view.keys, view.scrolls),
                (0, 0, 0, 0, 0)
            );
            assert_eq!(view.editor.read(cx).text(), "kept");
        })
        .unwrap();
}

#[gpui::test]
fn ok_mouse_gesture_is_consumed_and_next_intentional_click_is_not(cx: &mut TestAppContext) {
    let handle = setup(cx, false);
    explain(handle.into(), cx);
    let position = ok_center(handle.into(), cx);
    down(handle.into(), position, cx);
    assert!(pending(cx).is_some());
    assert_no_underlay_input(handle, cx);
    up(handle.into(), position, cx);
    assert!(pending(cx).is_none());
    assert_no_underlay_input(handle, cx);
    draw(handle.into(), cx);
    down(handle.into(), position, cx);
    up(handle.into(), position, cx);
    handle
        .read_with(cx, |view, _| {
            assert_eq!((view.downs, view.ups, view.clicks), (1, 1, 1))
        })
        .unwrap();
}

#[gpui::test]
fn background_click_scroll_and_interrupted_drag_do_not_reach_underlay(cx: &mut TestAppContext) {
    let handle = setup(cx, false);
    explain(handle.into(), cx);
    let outside = point(px(12.), px(12.));
    down(handle.into(), outside, cx);
    up(handle.into(), outside, cx);
    send(
        handle.into(),
        ScrollWheelEvent {
            position: outside,
            delta: ScrollDelta::Lines(point(0., 3.)),
            ..Default::default()
        },
        cx,
    );
    assert!(pending(cx).is_some());
    assert_no_underlay_input(handle, cx);
    let position = ok_center(handle.into(), cx);
    down(handle.into(), position, cx);
    send(
        handle.into(),
        MouseMoveEvent {
            position: outside,
            pressed_button: Some(MouseButton::Left),
            modifiers: Modifiers::default(),
        },
        cx,
    );
    up(handle.into(), outside, cx);
    assert!(pending(cx).is_some(), "dragging off OK must not dismiss");
    assert_no_underlay_input(handle, cx);
    down(handle.into(), position, cx);
    up(handle.into(), position, cx);
    assert!(pending(cx).is_none());
    assert_no_underlay_input(handle, cx);
}

#[gpui::test]
fn enter_space_escape_consume_held_downs_and_release_before_restoring_focus(
    cx: &mut TestAppContext,
) {
    let handle = setup(cx, false);
    for key in ["enter", "space", "escape"] {
        explain(handle.into(), cx);
        let keystroke = Keystroke::parse(key).unwrap();
        send(
            handle.into(),
            KeyDownEvent {
                keystroke: keystroke.clone(),
                is_held: false,
            },
            cx,
        );
        for _ in 0..3 {
            send(
                handle.into(),
                KeyDownEvent {
                    keystroke: keystroke.clone(),
                    is_held: true,
                },
                cx,
            );
            assert!(
                pending(cx).is_some(),
                "a held key cannot restore the root early"
            );
        }
        assert_no_underlay_input(handle, cx);
        send(handle.into(), KeyUpEvent { keystroke }, cx);
        assert!(pending(cx).is_none());
        assert_no_underlay_input(handle, cx);
        draw(handle.into(), cx);
    }
    send(
        handle.into(),
        KeyDownEvent {
            keystroke: Keystroke::parse("a").unwrap(),
            is_held: false,
        },
        cx,
    );
    send(
        handle.into(),
        KeyUpEvent {
            keystroke: Keystroke::parse("a").unwrap(),
        },
        cx,
    );
    handle
        .read_with(cx, |view, _| assert_eq!(view.keys, 2))
        .unwrap();
}

#[gpui::test]
fn text_input_and_repeated_quit_are_shielded_then_editor_input_resumes(cx: &mut TestAppContext) {
    let handle = setup(cx, true);
    explain(handle.into(), cx);
    let prompt = pending(cx);
    for _ in 0..3 {
        cx.simulate_keystrokes(handle.into(), "x");
        send(
            handle.into(),
            KeyUpEvent {
                keystroke: Keystroke::parse("x").unwrap(),
            },
            cx,
        );
        cx.dispatch_action(handle.into(), crate::shutdown::Quit);
        assert_eq!(pending(cx), prompt);
        assert!(!cx.read(crate::shutdown::requested));
        assert_no_underlay_input(handle, cx);
    }
    dismiss_with_input(cx);
    assert!(pending(cx).is_none());
    assert_no_underlay_input(handle, cx);
    draw(handle.into(), cx);
    cx.simulate_keystrokes(handle.into(), "x");
    handle
        .read_with(cx, |view, cx| {
            assert!(view.editor.read(cx).text().contains('x'))
        })
        .unwrap();
}

#[gpui::test]
fn unrelated_prompt_uses_default_builder_after_owned_prompt_creation(cx: &mut TestAppContext) {
    let handle = setup(cx, false);
    explain(handle.into(), cx);
    assert!(
        !cx.has_pending_prompt(),
        "Quit uses its owned view, not the native/default test prompt"
    );
    dismiss_with_input(cx);
    let answer = handle
        .update(cx, |_, window, cx| {
            window.prompt(
                gpui::PromptLevel::Info,
                "Unrelated prompt",
                Some("Unchanged default behavior"),
                &["Continue"],
                cx,
            )
        })
        .unwrap();
    assert_eq!(
        cx.pending_prompt(),
        Some((
            "Unrelated prompt".into(),
            "Unchanged default behavior".into()
        ))
    );
    cx.simulate_prompt_answer("Continue");
    cx.run_until_parked();
    drop(answer);
    assert!(!cx.has_pending_prompt());
}

#[gpui::test]
fn real_home_cards_do_not_activate_behind_quit_refusal_and_root_is_preserved(
    cx: &mut TestAppContext,
) {
    cx.update(crate::shutdown::init);
    let home = cx.add_window(|window, cx| {
        crate::shutdown::guard_window(window, cx);
        crate::home::Home::new(window, cx)
    });
    home.update(cx, |_, window, _| window.activate_window())
        .unwrap();
    let original = home.root(cx).unwrap();
    draw(home.into(), cx);
    let card = VisualTestContext::from_window(home.into(), cx)
        .debug_bounds("homeTool_textTools")
        .expect("actual Home Text Tools card")
        .center();
    explain(home.into(), cx);
    down(home.into(), card, cx);
    up(home.into(), card, cx);
    assert_eq!(cx.read(|cx| cx.windows().len()), 1);
    assert!(pending(cx).is_some());
    let ok = ok_center(home.into(), cx);
    down(home.into(), ok, cx);
    up(home.into(), ok, cx);
    assert!(pending(cx).is_none());
    assert_eq!(
        cx.read(|cx| cx.windows().len()),
        1,
        "OK cannot open a Home card"
    );
    assert_eq!(home.root(cx).unwrap().entity_id(), original.entity_id());
    draw(home.into(), cx);
    down(home.into(), card, cx);
    up(home.into(), card, cx);
    assert_eq!(
        cx.read(|cx| cx.windows().len()),
        2,
        "the next intended card click still works"
    );
}

#[gpui::test]
fn default_prompt_builder_is_restored_after_construction_panics(cx: &mut TestAppContext) {
    let handle = setup(cx, false);
    let answer = handle
        .update(cx, |_, window, cx| {
            cx.default_global::<super::super::State>()
                .panic_prompt_construction = true;
            let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                show("Synthetic refusal", window, cx)
            }));
            assert!(failed.is_err());
            window.prompt(
                gpui::PromptLevel::Info,
                "After construction error",
                None,
                &["Continue"],
                cx,
            )
        })
        .unwrap();
    assert_eq!(cx.pending_prompt().unwrap().0, "After construction error");
    cx.simulate_prompt_answer("Continue");
    cx.run_until_parked();
    drop(answer);
    assert!(pending(cx).is_none());
    explain(handle.into(), cx);
    dismiss_with_input(cx);
}

#[gpui::test]
fn pointer_press_started_before_prompt_cannot_complete_an_underlay_click(cx: &mut TestAppContext) {
    let handle = setup(cx, false);
    let position = point(px(40.), px(40.));
    down(handle.into(), position, cx);
    handle
        .read_with(cx, |view, _| {
            assert_eq!((view.downs, view.ups, view.clicks), (1, 0, 0))
        })
        .unwrap();
    explain(handle.into(), cx);
    up(handle.into(), position, cx);
    assert!(pending(cx).is_some());
    handle
        .read_with(cx, |view, _| {
            assert_eq!((view.downs, view.ups, view.clicks), (1, 0, 0))
        })
        .unwrap();
    let ok = ok_center(handle.into(), cx);
    down(handle.into(), ok, cx);
    up(handle.into(), ok, cx);
    assert!(pending(cx).is_none());
    handle
        .read_with(cx, |view, _| {
            assert_eq!((view.downs, view.ups, view.clicks), (1, 0, 0))
        })
        .unwrap();
}

#[gpui::test]
fn mouse_dismissal_waits_for_observed_held_key_and_consumes_its_release(cx: &mut TestAppContext) {
    let handle = setup(cx, false);
    for key in ["enter", "space", "x"] {
        explain(handle.into(), cx);
        let keystroke = Keystroke::parse(key).unwrap();
        send(
            handle.into(),
            KeyDownEvent {
                keystroke: keystroke.clone(),
                is_held: false,
            },
            cx,
        );
        let ok = ok_center(handle.into(), cx);
        down(handle.into(), ok, cx);
        up(handle.into(), ok, cx);
        assert!(pending(cx).is_some());
        send(
            handle.into(),
            KeyDownEvent {
                keystroke: keystroke.clone(),
                is_held: true,
            },
            cx,
        );
        assert_no_underlay_input(handle, cx);
        send(handle.into(), KeyUpEvent { keystroke }, cx);
        assert!(pending(cx).is_none());
        assert_no_underlay_input(handle, cx);
        draw(handle.into(), cx);
    }
}

#[gpui::test]
fn modifier_release_finishes_pending_dismissal_without_key_up(cx: &mut TestAppContext) {
    let handle = setup(cx, false);
    explain(handle.into(), cx);
    let modifiers = Modifiers {
        control: true,
        ..Default::default()
    };
    send(
        handle.into(),
        gpui::ModifiersChangedEvent {
            modifiers,
            ..Default::default()
        },
        cx,
    );
    let ok = ok_center(handle.into(), cx);
    send(
        handle.into(),
        MouseDownEvent {
            position: ok,
            button: MouseButton::Left,
            click_count: 1,
            modifiers,
            ..Default::default()
        },
        cx,
    );
    send(
        handle.into(),
        MouseUpEvent {
            position: ok,
            button: MouseButton::Left,
            click_count: 1,
            modifiers,
        },
        cx,
    );
    assert!(pending(cx).is_some());
    assert_no_underlay_input(handle, cx);
    send(handle.into(), gpui::ModifiersChangedEvent::default(), cx);
    assert!(pending(cx).is_none());
    assert_no_underlay_input(handle, cx);
}

#[gpui::test]
fn focus_loss_abandons_stale_hold_and_next_deliberate_gesture_dismisses(cx: &mut TestAppContext) {
    let handle = setup(cx, false);
    explain(handle.into(), cx);
    send(
        handle.into(),
        KeyDownEvent {
            keystroke: Keystroke::parse("x").unwrap(),
            is_held: false,
        },
        cx,
    );
    let ok = ok_center(handle.into(), cx);
    down(handle.into(), ok, cx);
    up(handle.into(), ok, cx);
    assert!(pending(cx).is_some());
    VisualTestContext::from_window(handle.into(), cx).deactivate_window();
    handle
        .update(cx, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    draw(handle.into(), cx);
    assert!(
        pending(cx).is_some(),
        "focus changes cannot silently dismiss"
    );
    let ok = ok_center(handle.into(), cx);
    down(handle.into(), ok, cx);
    up(handle.into(), ok, cx);
    assert!(pending(cx).is_none());
    assert_no_underlay_input(handle, cx);
}

#[gpui::test]
fn dismissal_without_prior_focus_refreshes_before_the_next_real_quit_key(cx: &mut TestAppContext) {
    struct Unfocused;
    impl Render for Unfocused {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full()
        }
    }
    cx.update(crate::shutdown::init);
    let handle = cx.add_window(|window, cx| {
        crate::shutdown::guard_window(window, cx);
        Unfocused
    });
    handle
        .update(cx, |_, window, cx| assert!(window.focused(cx).is_none()))
        .unwrap();
    explain(handle.into(), cx);
    send(
        handle.into(),
        KeyDownEvent {
            keystroke: Keystroke::parse("enter").unwrap(),
            is_held: false,
        },
        cx,
    );
    send(
        handle.into(),
        KeyUpEvent {
            keystroke: Keystroke::parse("enter").unwrap(),
        },
        cx,
    );
    assert!(pending(cx).is_none());
    // No test draw or direct action injection may repair the tree here.
    send(
        handle.into(),
        KeyDownEvent {
            keystroke: Keystroke::parse(crate::shutdown::QUIT_KEYSTROKE).unwrap(),
            is_held: false,
        },
        cx,
    );
    assert!(cx.read(crate::shutdown::requested));
    assert_eq!(cx.read(crate::shutdown::quit_calls), 1);
}

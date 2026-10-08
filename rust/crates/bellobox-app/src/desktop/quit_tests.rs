use super::*;
use gpui::TestAppContext;
struct Home;
impl gpui::Render for Home {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui::IntoElement {
        div()
    }
}
#[gpui::test]
fn qr_save_dialog_survives_host_close_and_quit_refusal_preserves_its_output(
    cx: &mut TestAppContext,
) {
    cx.update(crate::shutdown::init);
    let qr = cx.add_window(|window, cx| {
        crate::shutdown::guard_window(window, cx);
        BelloBox::new_for("qr".into(), "bounded quit fixture".into(), window, cx)
    });
    let home = cx.add_window(|window, cx| {
        crate::shutdown::guard_window(window, cx);
        Home
    });
    cx.run_until_parked();
    qr.update(cx, |qr, _, cx| qr.save_qr(cx)).unwrap();
    qr.update(cx, |_, window, cx| {
        crate::shutdown::close_window(window, cx)
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(cx.read(|cx| cx.windows().len()), 1);
    cx.dispatch_action(home.into(), crate::shutdown::Quit);
    assert!(!cx.read(crate::shutdown::requested));
    assert_eq!(
        crate::shutdown::pending_refusal(cx).unwrap().1,
        "Finish or cancel the QR save first."
    );
    crate::shutdown::dismiss_refusal(cx);
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("retained-qr.png");
    cx.simulate_new_path_selection(|_| Some(output.clone()));
    cx.run_until_parked();
    assert_eq!(
        std::fs::read(&output).unwrap(),
        bellobox_core::qr::png("bounded quit fixture").unwrap()
    );
    assert!(!cx.read(crate::shutdown::requested));
    cx.dispatch_action(home.into(), crate::shutdown::Quit);
    assert_eq!(cx.read(crate::shutdown::quit_calls), 1);
    assert!(output.is_file());
}

#[gpui::test]
fn qr_save_cancel_and_invalid_input_leave_no_quit_blocker(cx: &mut TestAppContext) {
    cx.update(crate::shutdown::init);
    let qr = cx.add_window(|window, cx| {
        crate::shutdown::guard_window(window, cx);
        BelloBox::new_for("qr".into(), "quit cancellation fixture".into(), window, cx)
    });
    cx.run_until_parked();
    qr.update(cx, |qr, _, cx| qr.save_qr(cx)).unwrap();
    cx.simulate_new_path_selection(|_| None);
    cx.run_until_parked();
    qr.update(cx, |qr, _, cx| {
        qr.input
            .update(cx, |editor, cx| editor.set_text(String::new(), cx));
        qr.save_qr(cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.dispatch_action(qr.into(), crate::shutdown::Quit);
    assert_eq!(cx.read(crate::shutdown::quit_calls), 1);
}

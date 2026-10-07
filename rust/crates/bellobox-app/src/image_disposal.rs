//! Retire rendered images only after GPUI returns borrowed windows to the app.
use gpui::{App, RenderImage};
use std::sync::Arc;

pub(crate) fn drop_image(image: Arc<RenderImage>, cx: &mut App) {
    // GPUI 0.2.2 temporarily removes a window from App.windows during its
    // callbacks. Passing None to App::drop_image there would miss that atlas.
    // Defer independently of the view so close/release cannot cancel cleanup.
    cx.defer(move |cx| {
        #[cfg(test)]
        record_disposal(image.id, cx);
        cx.drop_image(image, None);
    });
}

#[cfg(test)]
#[derive(Default)]
pub(crate) struct DisposalLog(pub Vec<(gpui::ImageId, usize)>);
#[cfg(test)]
impl gpui::Global for DisposalLog {}

#[cfg(test)]
fn record_disposal(image: gpui::ImageId, cx: &mut App) {
    use gpui::AppContext;
    if !cx.has_global::<DisposalLog>() {
        return;
    }
    // Observe the real disposal call, without replacing it with a mock. Every
    // live window must be back in App.windows at this point. GPUI's TestAtlas
    // is private, so this verifies dispatch/lifetime, not GPU byte accounting.
    let windows = cx.windows();
    for window in &windows {
        assert!(cx.update_window(*window, |_, _, _| ()).is_ok());
    }
    cx.global_mut::<DisposalLog>()
        .0
        .push((image, windows.len()));
}

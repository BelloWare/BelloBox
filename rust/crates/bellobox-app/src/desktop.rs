mod ai_updates;
mod permissions;
pub(crate) mod qr_jobs;
mod snippets;
use bello_workbench_ui::{EditorAppearance, EditorEvent, EditorView};
use bellobox_core::{
    launcher,
    settings::{Settings, config_dir},
};
use gpui::{
    App, Application, Bounds, ClipboardItem, Context, Entity, Image, ImageFormat, Subscription,
    TitlebarOptions, Window, WindowBounds, WindowOptions, div, img, prelude::*, px, rgb, size,
};
use std::{
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

struct BelloBox {
    selected: String,
    snippets: Option<snippets::SnippetUi>,
    permissions: Option<permissions::PermissionUi>,
    input: Entity<EditorView>,
    second: Entity<EditorView>,
    output: Entity<EditorView>,
    text_category: usize,
    controls: crate::tool_controls::ToolControls,
    open_menu: Option<&'static str>,
    menu_index: usize,
    initial_text: String,
    status: String,
    error: Option<String>,
    warning: bool,
    busy: bool,
    jobs: crate::session::SessionJobs,
    qr: Option<Arc<Image>>,
    qr_saves: crate::session::SessionJobs,
    qr_save_status: bool,
    _subscriptions: Vec<Subscription>,
    #[cfg(target_os = "macos")]
    updater: Option<bello_platform::macos_native::SparkleUpdater>,
}
pub(crate) fn perf(event: &str, micros: u128) {
    if let Some(path) = std::env::var_os("BELLO_PERF_LOG") {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(
                f,
                "{}",
                serde_json::json!({"app":"bellobox","event":event,"cpu_microseconds":micros,"note":"CPU callback timing, not GPU presentation"})
            );
        }
    }
}
fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}
impl BelloBox {
    fn new_for(command: String, text: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let fixture = command;
        let initial_text = text.clone();
        let controls = crate::tool_controls::ToolControls::new(&fixture, &text);
        let input = cx.new(|cx| EditorView::new(text, window, cx));
        let second = cx.new(|cx| EditorView::new(String::new(), window, cx));
        let output = cx.new(|cx| {
            let mut editor = EditorView::new(String::new(), window, cx);
            editor.set_read_only(true, cx);
            editor
        });
        let subscription = cx.subscribe(&input, |this, _, event: &EditorEvent, cx| {
            if matches!(event, EditorEvent::Changed)
                && !["ai", "http", "generate"].contains(&this.selected.as_str())
            {
                this.run_tool(cx);
            }
        });
        let options_subscription = cx.subscribe(&second, |this, _, event: &EditorEvent, cx| {
            if matches!(event, EditorEvent::Changed)
                && !["ai", "http", "generate"].contains(&this.selected.as_str())
            {
                this.run_tool(cx);
            }
        });
        let status = String::new();
        let mut app = Self {
            selected: fixture,
            snippets: None,
            permissions: None,
            input,
            second,
            output,
            text_category: 0,
            controls,
            open_menu: None,
            menu_index: 0,
            initial_text,
            status,
            error: None,
            warning: false,
            busy: false,
            jobs: crate::session::SessionJobs::default(),
            qr: None,
            qr_saves: crate::session::SessionJobs::default(),
            qr_save_status: false,
            _subscriptions: vec![subscription, options_subscription],
            #[cfg(target_os = "macos")]
            updater: None,
        };
        let palette = crate::theme::for_window(window);
        for (index, editor) in [&app.input, &app.second, &app.output]
            .into_iter()
            .enumerate()
        {
            editor.update(cx, |editor, cx| {
                if matches!(app.selected.as_str(), "subnet" | "chmod" | "numberBase") && index == 0
                {
                    editor.set_compact(true, cx);
                }
                let mut appearance = EditorAppearance::plain();
                appearance.font_family =
                    if ["qr", "textTools", "ai"].contains(&app.selected.as_str()) {
                        crate::theme::ui_font().into()
                    } else {
                        "monospace".into()
                    };
                appearance.font_size = if ["qr", "textTools", "ai"].contains(&app.selected.as_str())
                    || (matches!(app.selected.as_str(), "subnet" | "chmod" | "numberBase")
                        && index == 0)
                {
                    13.
                } else {
                    12.
                };
                appearance.line_height = 18.;
                if matches!(app.selected.as_str(), "subnet" | "chmod" | "numberBase") && index == 0
                {
                    // Keep one full text row inside the 38 px compact field.
                    appearance.padding_x = 0.;
                    appearance.padding_y = 0.;
                }
                appearance.text = palette.primary;
                appearance.caret = palette.accent;
                appearance.selection = palette.accent.opacity(0.18);
                appearance.background = None;
                editor.set_appearance(appearance, cx);
            });
        }
        crate::shutdown::guard_quit(window, cx, |this, _, cx| {
            if this.snippets.as_ref().is_some_and(|ui| ui.deleting) {
                // Keep the existing native delete confirmation in place.
                this.status = "Dismiss the snippet dialog first.".into();
                cx.notify();
                crate::shutdown::QuitAdmission::Refused
            } else if this.busy {
                crate::shutdown::QuitAdmission::Explain("Finish this tool's current action.")
            } else {
                crate::shutdown::QuitAdmission::Ready
            }
        });
        app.init_snippets(window, cx);
        app.init_permissions(window, cx);
        if matches!(app.selected.as_str(), "subnet" | "chmod" | "numberBase") {
            app.input.read(cx).focus(window);
        }
        if app.selected == "ai" {
            app.input.update(cx, |e, cx| e.set_read_only(true, cx));
        }
        if app.selected == "textTools" {
            app.second
                .update(cx, |e, cx| e.set_text("upper".into(), cx));
        }
        if app.selected == "updates" {
            app.check_updates(cx);
        } else if app.selected == "setup" {
            app.status="The original setup guide is not ported yet. Local tools are available without granting selection or capture permissions.".into();
        } else if app.selected == "settings" {
            app.status="The original Settings panels are being ported. Rust preferences remain separate from Swift. Provider credentials are currently runtime-only; no key is written to settings.".into();
        } else {
            app.run_tool(cx);
        }
        app
    }
    fn run_tool(&mut self, cx: &mut Context<Self>) {
        let revision = self.jobs.begin();
        self.clear_permission_preview(cx);
        self.qr_saves.cancel();
        self.qr_save_status = false;
        self.qr = None;
        self.error = None;
        self.warning = false;
        let id = self.selected.clone();
        let input = self.input.read(cx).text().to_string();
        let second = self
            .controls
            .second(&self.selected, self.second.read(cx).text());
        if id == "ai"
            || (matches!(
                id.as_str(),
                "stringEscape" | "subnet" | "chmod" | "numberBase"
            ) && crate::tool_controls::source_input_is_idle(&input))
            || input.is_empty()
                && !["worldClock", "textTools", "generate", "screenshot"].contains(&id.as_str())
                && !(id == "listSet" && !second.is_empty())
        {
            self.output
                .update(cx, |e, cx| e.set_text(String::new(), cx));
            self.qr = None;
            self.busy = false;
            self.status.clear();
            cx.notify();
            return;
        }
        if id == "qr" {
            if let Err(error) = qr_jobs::validate(&input) {
                self.busy = false;
                self.status = error;
                cx.notify();
                return;
            }
            self.busy = true;
            self.status = "Generating QR code…".into();
            cx.notify();
            let cancellation = self.jobs.cancellation();
            let task = cx
                .background_executor()
                .spawn(async move { qr_jobs::generate(&input, &cancellation) });
            cx.spawn(async move |this, cx| {
                let result = task.await;
                let _ = this.update(cx, |this, cx| {
                    if !this.jobs.accepts(revision) {
                        return;
                    }
                    let Some(result) = result else { return };
                    this.busy = false;
                    let status = match result {
                        Ok(result) => {
                            this.qr =
                                Some(Arc::new(Image::from_bytes(ImageFormat::Png, result.png)));
                            String::new()
                        }
                        Err(error) => error,
                    };
                    if !this.qr_save_status {
                        this.status = status;
                    }
                    cx.notify();
                });
            })
            .detach();
            return;
        }
        if matches!(
            id.as_str(),
            "stringEscape" | "subnet" | "chmod" | "numberBase"
        ) {
            self.output
                .update(cx, |editor, cx| editor.set_text(String::new(), cx));
        }
        self.busy = true;
        self.status = "Working locally…".into();
        cx.notify();
        let snippet = if cfg!(feature = "developer-tools") {
            self.snippet_snapshot()
        } else {
            None
        };
        #[cfg(feature = "developer-tools")]
        let list_options = (
            self.controls.value("mode").to_owned(),
            self.controls.value("matching").to_owned(),
        );
        #[cfg(feature = "developer-tools")]
        let number_base_options = self.controls.number_base_options();
        let task = cx.background_executor().spawn(async move {
            #[cfg(feature = "developer-tools")]
            if id == "numberBase" {
                return bellobox_core::developer::number_base::run(
                    &input,
                    number_base_options.0,
                    number_base_options.1,
                );
            }
            #[cfg(feature = "developer-tools")]
            if id == "listSet" {
                return bellobox_core::developer::list_set::run(
                    &input,
                    &second,
                    &list_options.0,
                    &list_options.1,
                );
            }
            if let Some(session) = snippet {
                session.render(&input)
            } else {
                crate::execute(&id, &input, &second)
            }
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if !this.jobs.accepts(revision) {
                    return;
                }
                this.busy = false;
                let (output, status) = match result {
                    Ok(value) => {
                        this.warning = this.selected == "jsonSchema"
                            && !value.starts_with("Valid against this schema.");
                        this.error = None;
                        let status = if this.selected == "listSet" {
                            format!(
                                "{} items · stable order · blank lines and duplicates removed",
                                if value.is_empty() {
                                    0
                                } else {
                                    value.split('\n').count()
                                }
                            )
                        } else if this.selected == "numberBase" {
                            this.controls.number_base_status()
                        } else if this.selected == "chmod" {
                            "Permissions preview · no files are changed".into()
                        } else if this.selected == "subnet" {
                            "IPv4 subnet · calculated locally".into()
                        } else if this.selected == "stringEscape" {
                            format!("{} · literal text only", this.controls.value("mode"))
                        } else if this.selected == "snippets" {
                            "Template preview · date and timestamp use UTC".into()
                        } else if this.warning {
                            "Validation issues · paths use JSON Pointer".into()
                        } else {
                            "Ready. Input is kept in memory.".into()
                        };
                        (value, status)
                    }
                    Err(error) => {
                        this.error = Some(error);
                        (String::new(), "This operation needs attention.".into())
                    }
                };
                this.sync_permission_preview(cx);
                this.output
                    .update(cx, |editor, cx| editor.set_text(output, cx));
                this.status = status;
                cx.notify();
            });
        })
        .detach();
    }
    fn send_ai(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let input = self.input.read(cx).text().to_string();
        let instruction = self.second.read(cx).text().to_string();
        let request = crate::transport::config_from_env().and_then(|c| {
            bellobox_core::ai::request(
                &c,
                &std::env::var("BELLOBOX_AI_KEY").unwrap_or_default(),
                &instruction,
                &input,
            )
        });
        let request = match request {
            Ok(r) => r,
            Err(e) => {
                self.status = e;
                cx.notify();
                return;
            }
        };
        let revision = self.jobs.begin();
        self.busy = true;
        self.status = "Sending to your configured AI endpoint…".into();
        self.output
            .update(cx, |e, cx| e.set_text(String::new(), cx));
        let (tx, rx) = std::sync::mpsc::channel();
        let cancellation = self.jobs.cancellation();
        std::thread::spawn(move || {
            let result = crate::transport::send_cancellable(
                request,
                |chunk| {
                    let _ = tx.send(Ok(Some(chunk.to_owned())));
                },
                cancellation,
            );
            let _ = tx.send(result.map(|_| None));
        });
        cx.spawn(async move |this, cx| {
            let mut text = String::new();
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(40))
                    .await;
                let batch = ai_updates::drain(&rx, &mut text);
                let keep = this
                    .update(cx, |this, cx| {
                        if !this.jobs.accepts(revision) {
                            return false;
                        }
                        if batch.changed {
                            this.output.update(cx, |e, cx| e.set_text(text.clone(), cx));
                        }
                        if batch.done {
                            this.busy = false;
                            this.status = batch.error.unwrap_or_else(|| {
                                "AI response complete. Nothing was replaced in another app.".into()
                            });
                        }
                        if batch.changed || batch.done {
                            cx.notify();
                        }
                        !batch.done
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }
    fn capture_screen(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let directory = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let dialog = cx.prompt_for_new_path(&directory, Some("BelloBox Screenshot.png"));
        cx.spawn(async move |this,cx| {
            let path=match dialog.await {
                Ok(Ok(Some(path)))=>path,
                Ok(Ok(None))=>return,
                _=>{let _=this.update(cx,|this,cx|{this.status="The system save dialog could not be opened.".into();cx.notify();});return;}
            };
            let revision=match this.update(cx,|this,cx|{let token=this.jobs.begin();this.busy=true;this.status="Capturing the full screen to the chosen new file…".into();cx.notify();token}){Ok(value)=>value,Err(_)=>return};
            let task=cx.background_executor().spawn(async move {
                let capture=bello_platform::Platform::new().capture_screenshot(&path).map_err(|e|e.to_string())?;
                let bytes=std::fs::read(&capture.path).map_err(|e|e.to_string())?;
                Ok::<_,String>((capture,bytes))
            });
            let result=task.await;
            let _=this.update(cx,|this,cx|{if !this.jobs.accepts(revision){return;}this.busy=false;
                match result {
                    Ok((capture,bytes))=>{this.qr=Some(Arc::new(Image::from_bytes(ImageFormat::Png,bytes)));this.status=format!("Saved full-screen capture: {}",capture.path.display());this.output.update(cx,|e,cx|e.set_text(format!("{} × {} PNG\n{}\n\nThis preview does not yet include area selection or annotations.",capture.width,capture.height,capture.path.display()),cx));},
                    Err(e)=>this.status=e,
                }cx.notify();
            });
        }).detach();
    }
    fn ocr_image(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let dialog = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose a local image for offline OCR".into()),
        });
        cx.spawn(async move |this, cx| {
            let path = match dialog.await {
                Ok(Ok(Some(paths))) => match paths.into_iter().next() {
                    Some(path) => path,
                    None => return,
                },
                Ok(Ok(None)) => return,
                _ => {
                    let _ = this.update(cx, |this, cx| {
                        this.status = "The system file dialog could not be opened.".into();
                        cx.notify();
                    });
                    return;
                }
            };
            let revision = match this.update(cx, |this, cx| {
                let token = this.jobs.begin();
                this.busy = true;
                this.status = "Recognizing locally. No image is uploaded.".into();
                cx.notify();
                token
            }) {
                Ok(value) => value,
                Err(_) => return,
            };
            let task = cx.background_executor().spawn(async move {
                bello_platform::Platform::new()
                    .recognize_text(&path, None)
                    .map_err(|e| e.to_string())
            });
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if !this.jobs.accepts(revision) {
                    return;
                }
                this.busy = false;
                match result {
                    Ok(text) => {
                        this.output.update(cx, |e, cx| e.set_text(text, cx));
                        this.status = "Local OCR complete.".into();
                    }
                    Err(e) => this.status = e,
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn check_updates(&mut self, cx: &mut Context<Self>) {
        #[cfg(target_os = "macos")]
        {
            let result = (|| {
                if self.updater.is_none() {
                    self.updater = Some(bello_platform::macos_native::SparkleUpdater::new()?);
                }
                self.updater
                    .as_mut()
                    .expect("updater initialized")
                    .check_for_updates()
            })();
            self.status = match result {
                Ok(()) => "Sparkle accepted the check. Follow its native update dialog.".into(),
                Err(e) => e.to_string(),
            };
        }
        #[cfg(not(target_os = "macos"))]
        {
            self.status =
                "Sparkle updates are macOS-only. Linux release updating is not implemented.".into();
        }
        cx.notify();
    }
    fn save_qr(&mut self, cx: &mut Context<Self>) {
        // The dialog and worker retain exactly the text present at this click.
        // Editing cancels only publication of the save status, never retargets
        // an explicit save to different text or silently cancels its disk write.
        let input = self.input.read(cx).text().to_string();
        if let Err(error) = qr_jobs::validate(&input) {
            self.status = error;
            cx.notify();
            return;
        }
        let quit_blocker = crate::shutdown::block_quit(cx, "Finish or cancel the QR save first.");
        let revision = self.qr_saves.begin();
        self.qr_save_status = true;
        self.status = "Choose where to save the QR image…".into();
        cx.notify();
        let path = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let dialog = cx.prompt_for_new_path(&path, Some("BelloBox QR.png"));
        let executor = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let message = match dialog.await {
                Ok(Ok(Some(path))) => {
                    executor
                        .spawn(async move {
                            let _quit_blocker = quit_blocker;
                            match bellobox_core::qr::png(&input)
                                .and_then(|bytes| crate::save_new(&path, &bytes))
                            {
                                Ok(()) => format!("Saved {}", path.display()),
                                Err(e) => {
                                    format!("Not saved: {e}. Existing files are never overwritten.")
                                }
                            }
                        })
                        .await
                }
                Ok(Ok(None)) => "Save cancelled.".into(),
                _ => "The system save dialog could not be opened.".into(),
            };
            let _ = this.update(cx, |this, cx| {
                if !this.qr_saves.accepts(revision) {
                    return;
                }
                this.status = message;
                cx.notify();
            });
        })
        .detach();
    }
    fn example(&mut self, cx: &mut Context<Self>) {
        let example = match self.selected.as_str() {
            "qr" => "https://belloware.com",
            "worldClock" => "2026-10-04T12:00:00Z",
            "textTools" => "helloWorld café\nBelloBox tools",
            _ => {
                #[cfg(feature = "developer-tools")]
                {
                    bellobox_core::developer::catalog()
                        .iter()
                        .find(|t| t.id == self.selected)
                        .map(|t| t.example)
                        .unwrap_or("")
                }
                #[cfg(not(feature = "developer-tools"))]
                {
                    ""
                }
            }
        };
        self.controls = crate::tool_controls::ToolControls::new(&self.selected, example);
        self.input
            .update(cx, |e, cx| e.set_text(example.into(), cx));
        let options = match self.selected.as_str() {
            "textTools" => "snake",
            "worldClock" => "UTC,America/Los_Angeles,Europe/London,Asia/Tokyo",
            _ => {
                #[cfg(feature = "developer-tools")]
                {
                    bellobox_core::developer::example_options(&self.selected)
                }
                #[cfg(not(feature = "developer-tools"))]
                {
                    ""
                }
            }
        };
        self.second
            .update(cx, |e, cx| e.set_text(options.into(), cx));
        self.run_tool(cx);
    }
}
// Source-preserving window renderers are below. Each tool owns an independent
// draft and opens separately from Home/the transient launcher.
fn button(
    id: impl Into<gpui::ElementId>,
    label: impl Into<gpui::SharedString>,
    p: crate::theme::Palette,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px(px(10.))
        .h(px(30.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(7.))
        .border_1()
        .border_color(p.separator)
        .bg(p.surface)
        .text_size(px(12.))
        .text_color(p.primary)
        .cursor_pointer()
        .hover(move |s| s.bg(p.accent.opacity(0.09)))
        .child(label.into())
}
fn link(
    id: impl Into<gpui::ElementId>,
    label: impl Into<gpui::SharedString>,
    p: crate::theme::Palette,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .text_size(px(11.))
        .text_color(p.accent)
        .cursor_pointer()
        .child(label.into())
}
fn badge(id: &str, size: f32, p: crate::theme::Palette) -> gpui::Div {
    div()
        .size(px(size))
        .flex_none()
        .rounded(px(size * 0.28))
        .bg(p.accent.opacity(0.11))
        .flex()
        .justify_center()
        .items_center()
        .child(crate::theme::tool_icon(id, size * 0.43, p))
}
fn editor_card(editor: Entity<EditorView>, height: f32, p: crate::theme::Palette) -> gpui::Div {
    div()
        .h(px(height))
        .flex_none()
        .p(px(9.))
        .rounded(px(10.))
        .border_1()
        .border_color(p.separator)
        .bg(p.well)
        .overflow_hidden()
        .child(editor)
}
impl BelloBox {
    fn input_actions(
        &self,
        label: &'static str,
        p: crate::theme::Palette,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .text_size(px(12.))
            .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(label))
            .child(div().flex_1())
            .when(self.selected != "snippets", |s| {
                s.child(
                    link("input-example", "Example", p)
                        .on_click(cx.listener(|this, _, _, cx| this.example(cx))),
                )
            })
            .child(
                link("input-paste", "Paste", p).on_click(cx.listener(|this, _, _, cx| {
                    if let Some(text) = cx.read_from_clipboard().and_then(|v| v.text()) {
                        match bellobox_core::validate_input(&text) {
                            Ok(()) => this.input.update(cx, |e, cx| e.set_text(text, cx)),
                            Err(e) => this.status = e,
                        };
                        cx.notify();
                    }
                })),
            )
            .child(
                link("input-clear", "Clear", p).on_click(cx.listener(|this, _, _, cx| {
                    this.input.update(cx, |e, cx| e.set_text(String::new(), cx))
                })),
            )
    }
    fn list_set_inputs(&self, p: crate::theme::Palette, cx: &mut Context<Self>) -> gpui::Div {
        let height = crate::tool_controls::list_set_editor_height(
            self.input.read(cx).text(),
            self.second.read(cx).text(),
        );
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(p.secondary)
                            .child("Two inputs · edit either side"),
                    )
                    .child(div().flex_1())
                    .child(
                        link("list-example", "Example", p)
                            .on_click(cx.listener(|this, _, _, cx| this.example(cx))),
                    )
                    .child(link("list-clear", "Clear", p).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.input.update(cx, |e, cx| e.set_text(String::new(), cx));
                            this.second
                                .update(cx, |e, cx| e.set_text(String::new(), cx));
                        },
                    ))),
            )
            .child(div().flex().gap(px(8.)).children((0usize..2).map(|side| {
                let label = if side == 0 {
                    "First list · one item per line"
                } else {
                    "List B · one item per line"
                };
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(p.secondary)
                                    .child(label),
                            )
                            .child(div().flex_1())
                            .child(link(("list-paste", side), "Paste", p).on_click(cx.listener(
                                move |this, _, _, cx| {
                                    if let Some(text) =
                                        cx.read_from_clipboard().and_then(|v| v.text())
                                    {
                                        match bellobox_core::validate_input(&text) {
                                            Ok(()) => {
                                                let editor = if side == 0 {
                                                    &this.input
                                                } else {
                                                    &this.second
                                                };
                                                editor.update(cx, |e, cx| e.set_text(text, cx));
                                            }
                                            Err(error) => this.status = error,
                                        }
                                        cx.notify();
                                    }
                                },
                            ))),
                    )
                    .child(editor_card(
                        if side == 0 {
                            self.input.clone()
                        } else {
                            self.second.clone()
                        },
                        height,
                        p,
                    ))
            })))
    }
    fn header(&self, p: crate::theme::Palette, popup: bool, cx: &mut Context<Self>) -> gpui::Div {
        let title = crate::theme::tool_title(&self.selected);
        let subtitle = if self.selected == "qr" {
            "Text to a scannable image, instantly"
        } else {
            crate::theme::tool_subtitle(&self.selected)
        };
        let header = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .pb(px(if popup { 10. } else { 0. }))
            .child(badge(&self.selected, 34., p))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(
                        div()
                            .text_size(px(16.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(p.secondary)
                            .child(subtitle),
                    ),
            )
            .child(div().flex_1());
        if popup {
            header
                .border_b_1()
                .border_color(p.separator)
                .child(
                    button("minimize", "−", p)
                        .w(px(28.))
                        .on_click(|_, window, _| window.minimize_window()),
                )
                .child(
                    button("close", "×", p)
                        .w(px(28.))
                        .on_click(|_, window, cx| crate::shutdown::close_window(window, cx)),
                )
        } else {
            header
                .p(px(14.))
                .child(button("new-window", "New Window", p).on_click(
                    cx.listener(|this, _, _, cx| open_tool(&this.selected, String::new(), cx)),
                ))
                .child(button("search-tools", "Search", p).on_click(cx.listener(
                    |this, _, _, cx| open_launcher(this.input.read(cx).text().into(), cx),
                )))
        }
    }
    fn choose(&mut self, option: &str, cx: &mut Context<Self>) {
        self.second
            .update(cx, |e, cx| e.set_text(option.into(), cx));
    }
    fn segmented(
        &self,
        choices: &[(&'static str, &'static str)],
        p: crate::theme::Palette,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let selected = self.second.read(cx).text();
        div()
            .flex()
            .gap(px(2.))
            .p(px(2.))
            .rounded(px(7.))
            .bg(p.surface)
            .border_1()
            .border_color(p.separator)
            .children(choices.iter().enumerate().map(|(i, (value, label))| {
                let value = *value;
                let chosen = selected == value || selected.is_empty() && i == 0;
                div()
                    .id(("choice", i))
                    .flex_1()
                    .h(px(26.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(5.))
                    .text_size(px(12.))
                    .text_color(if chosen { p.accent } else { p.secondary })
                    .when(chosen, |s| {
                        s.bg(p.surface).border_1().border_color(p.separator)
                    })
                    .cursor_pointer()
                    .child(*label)
                    .on_click(cx.listener(move |this, _, _, cx| this.choose(value, cx)))
            }))
    }
    fn source_option_menus(&self, p: crate::theme::Palette, cx: &mut Context<Self>) -> gpui::Div {
        div().flex().gap(px(8.)).children(
            crate::tool_controls::choices(&self.selected)
                .into_iter()
                .map(|spec| {
                    let id = spec.id;
                    let current = self.controls.value(id).to_owned();
                    let open = self.open_menu == Some(id);
                    let selected_index =
                        spec.choices.iter().position(|s| *s == current).unwrap_or(0);
                    let popup = div()
                        .id(gpui::ElementId::Name(format!("option-popup-{id}").into()))
                        .min_w(px(170.))
                        .p(px(4.))
                        .rounded(px(7.))
                        .border_1()
                        .border_color(p.separator)
                        .bg(p.surface)
                        .shadow_md()
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            this.open_menu = None;
                            cx.notify();
                        }))
                        .children(spec.choices.iter().enumerate().map(|(index, value)| {
                            let value = *value;
                            div()
                                .id((id, index))
                                .px(px(8.))
                                .h(px(28.))
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .rounded(px(4.))
                                .text_size(px(12.))
                                .cursor_pointer()
                                .when(self.menu_index == index, |s| s.bg(p.accent.opacity(0.1)))
                                .hover(move |s| s.bg(p.accent.opacity(0.12)))
                                .child(div().w(px(14.)).child(if value == current {
                                    "✓"
                                } else {
                                    ""
                                }))
                                .child(value)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.controls.select(&this.selected, id, value);
                                    this.open_menu = None;
                                    this.run_tool(cx);
                                    cx.stop_propagation();
                                    cx.notify();
                                }))
                        }));
                    div()
                        .relative()
                        .child(
                            button(
                                gpui::ElementId::Name(format!("option-menu-{id}").into()),
                                format!("{}: {}  ⌄", spec.label, current),
                                p,
                            )
                            .h(px(32.))
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.open_menu = if open { None } else { Some(id) };
                                    this.menu_index = selected_index;
                                    cx.stop_propagation();
                                    cx.notify();
                                },
                            )),
                        )
                        .when(open, |s| {
                            s.child(
                                gpui::deferred(
                                    gpui::anchored()
                                        .position_mode(gpui::AnchoredPositionMode::Local)
                                        .offset(gpui::point(px(0.), px(34.)))
                                        .snap_to_window()
                                        .child(popup),
                                )
                                .with_priority(20),
                            )
                        })
                }),
        )
    }
    fn render_qr(&self, p: crate::theme::Palette, cx: &mut Context<Self>) -> gpui::AnyElement {
        let bytes = self.input.read(cx).text().len();
        let remaining = 2000_i64 - bytes as i64;
        let qr_card = div()
            .w_full()
            .h_full()
            .min_h(px(160.))
            .max_h(px(300.))
            .rounded(px(12.))
            .bg(p.surface)
            .flex()
            .items_center()
            .justify_center()
            .when_some(self.qr.clone(), |s, image| {
                s.child(
                    div().p(px(14.)).bg(rgb(0xffffff)).rounded(px(8.)).child(
                        img(image)
                            .size(px(268.))
                            .object_fit(gpui::ObjectFit::Contain),
                    ),
                )
            })
            .when(self.qr.is_none(), |s| {
                s.child(
                    div()
                        .text_size(px(12.))
                        .text_color(p.secondary)
                        .child(if bytes == 0 {
                            "Enter text to encode"
                        } else {
                            "Could not generate a QR code"
                        }),
                )
            });
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(12.))
            .bg(p.bg)
            .text_color(p.primary)
            .font_family(crate::theme::ui_font())
            .child(self.header(p, true, cx))
            .child(
                div()
                    .flex_1()
                    .min_h(px(160.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(qr_card),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .text_size(px(12.))
                            .child(
                                div()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child("Encoded text"),
                            )
                            .child(div().flex_1())
                            .child(link("qr-clear", "Clear", p).on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.input.update(cx, |e, cx| e.set_text(String::new(), cx))
                                },
                            )))
                            .child(link("qr-paste", "Paste Text", p).on_click(cx.listener(
                                |this, _, _, cx| {
                                    if let Some(text) =
                                        cx.read_from_clipboard().and_then(|v| v.text())
                                    {
                                        this.input.update(cx, |e, cx| e.set_text(text, cx));
                                    }
                                },
                            ))),
                    )
                    .child(editor_card(self.input.clone(), 116., p))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .text_size(px(11.))
                            .text_color(if remaining < 0 { p.danger } else { p.secondary })
                            .child(format!("{bytes} / 2,000 bytes"))
                            .child(if remaining >= 0 {
                                format!("{remaining} bytes available")
                            } else {
                                format!("Remove at least {} bytes to create a QR code.", -remaining)
                            }),
                    ),
            )
            .child(
                div()
                    .min_h(px(20.))
                    .max_h(px(40.))
                    .text_size(px(11.))
                    .text_color(p.secondary)
                    .child(self.status.clone()),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        button("qr-save", "Save…", p)
                            .on_click(cx.listener(|this, _, _, cx| this.save_qr(cx))),
                    )
                    .child(
                        button("qr-copy", "Copy Image", p)
                            .bg(p.accent_fill)
                            .text_color(rgb(0xffffff))
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(image) = &this.qr {
                                    cx.write_to_clipboard(ClipboardItem::new_image(image));
                                    this.status = "Copied QR image.".into();
                                    cx.notify();
                                }
                            })),
                    ),
            )
            .into_any_element()
    }
    fn render_workbench(
        &mut self,
        p: crate::theme::Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let control = match self.selected.as_str() {
            "snippets" => self.snippet_controls(p, cx),
            "json" => self
                .segmented(
                    &[
                        ("pretty", "Pretty-print"),
                        ("minify", "Minify"),
                        ("validate", "Validate"),
                    ],
                    p,
                    cx,
                )
                .into_any_element(),
            "compare" => self.segmented(&[("", "Lines")], p, cx).into_any_element(),
            "convert" => self
                .segmented(
                    &[
                        ("json-yaml", "JSON → YAML"),
                        ("yaml-json", "YAML → JSON"),
                        ("csv-json", "CSV → JSON"),
                        ("json-csv", "JSON → CSV"),
                    ],
                    p,
                    cx,
                )
                .into_any_element(),
            "jsonFlatten" => self
                .segmented(&[("flatten", "Flatten"), ("unflatten", "Unflatten")], p, cx)
                .into_any_element(),
            _ => div().into_any_element(),
        };
        let control = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(control)
            .child(self.source_option_menus(p, cx));
        let paired =
            ["compare", "jsonMerge", "listSet", "jsonSchema"].contains(&self.selected.as_str());
        let has_second = !matches!(
            self.selected.as_str(),
            "json"
                | "snippets"
                | "calculator"
                | "color"
                | "jwt"
                | "unicode"
                | "subnet"
                | "chmod"
                | "numberBase"
                | "markdown"
                | "xmlJSON"
                | "generate"
                | "uuidInspect"
                | "sshKey"
                | "httpHeaders"
                | "http"
                | "bezier"
                | "convert"
                | "jsonFlatten"
                | "plist"
                | "sqlFormat"
                | "jsonLines"
                | "envFile"
                | "cookies"
                | "certificate"
                | "stringEscape"
        );
        let height = if matches!(self.selected.as_str(), "subnet" | "chmod" | "numberBase") {
            38.
        } else if self.selected == "stringEscape" {
            crate::tool_controls::string_literal_editor_height(self.input.read(cx).text())
        } else if ["time", "cron", "url"].contains(&self.selected.as_str()) {
            70.
        } else {
            150.
        };
        let input = if self.selected == "listSet" {
            self.list_set_inputs(p, cx).into_any_element()
        } else if paired {
            div()
                .flex()
                .gap(px(14.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(self.input_actions(self.controls.input_label(&self.selected), p, cx))
                        .child(editor_card(self.input.clone(), 190., p)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(
                            div()
                                .text_size(px(12.))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(crate::tool_controls::second_label(&self.selected)),
                        )
                        .child(editor_card(self.second.clone(), 190., p)),
                )
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(self.input_actions(self.controls.input_label(&self.selected), p, cx))
                .child(editor_card(self.input.clone(), height, p).when(
                    matches!(self.selected.as_str(), "subnet" | "chmod" | "numberBase"),
                    |field| {
                        field
                            .p(px(8.))
                            .rounded(px(8.))
                            .relative()
                            .when(self.input.read(cx).text().is_empty(), |field| {
                                field.child(
                                    div()
                                        .absolute()
                                        .left(px(9.))
                                        .top(px(9.))
                                        .text_size(px(13.))
                                        .line_height(px(18.))
                                        .font_family("monospace")
                                        .text_color(p.secondary)
                                        .child(match self.selected.as_str() {
                                            "chmod" => "755",
                                            "numberBase" => "9007199254740993",
                                            _ => "192.168.1.42/24",
                                        }),
                                )
                            })
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.input.read(cx).focus(window)
                                }),
                            )
                    },
                ))
                .when(has_second, |s| {
                    s.child(
                        div()
                            .text_size(px(11.))
                            .text_color(p.secondary)
                            .child(option_label(&self.selected)),
                    )
                    .child(editor_card(
                        self.second.clone(),
                        if self.selected == "regex" { 40. } else { 54. },
                        p,
                    ))
                })
                .into_any_element()
        };
        let result = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_size(px(12.))
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Result"),
                    )
                    .child(div().text_size(px(11.)).text_color(p.secondary).child(
                        if self.selected == "numberBase" && !self.busy {
                            self.controls.number_base_status()
                        } else {
                            (if self.busy {
                                "Working…"
                            } else if self.warning {
                                "Validation issues"
                            } else if self.selected == "chmod" {
                                "Permissions preview · no files are changed"
                            } else if self.selected == "subnet" {
                                "IPv4 subnet · calculated locally"
                            } else if self.selected == "json" {
                                "Numbers preserved"
                            } else {
                                "Processed locally"
                            })
                            .to_owned()
                        },
                    )),
            )
            .child(editor_card(self.output.clone(), 210., p));
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(p.bg)
            .text_color(p.primary)
            .font_family(crate::theme::ui_font())
            .child(self.header(p, false, cx))
            .child(div().h(px(1.)).bg(p.separator))
            .child(
                div()
                    .id("tool-content")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p(px(16.))
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .child(control)
                    .child(input)
                    .when(self.selected == "chmod", |s| {
                        s.child(self.permission_grid(p, cx))
                    })
                    .when(
                        self.selected == "chmod"
                            && !self.busy
                            && self.error.is_none()
                            && !self.output.read(cx).text().is_empty(),
                        |s| s.child(self.permission_preview(p)),
                    )
                    .when(self.selected == "snippets", |s| {
                        s.child(self.snippet_fields(p, window, cx))
                    })
                    .when_some(self.error.clone(), |s, error| {
                        s.child(
                            div()
                                .p(px(12.))
                                .rounded(px(10.))
                                .bg(p.danger.opacity(0.07))
                                .text_color(p.danger)
                                .text_size(px(13.))
                                .child(error),
                        )
                    })
                    .when(!self.output.read(cx).text().is_empty(), |s| s.child(result))
                    .when(
                        self.output.read(cx).text().is_empty()
                            && self.error.is_none()
                            && !self.busy,
                        |s| {
                            s.child(
                                div()
                                    .p(px(28.))
                                    .flex()
                                    .justify_center()
                                    .text_color(p.secondary)
                                    .text_size(px(13.))
                                    .child(if self.selected == "stringEscape" {
                                        crate::tool_controls::string_literal_empty_message(
                                            self.input.read(cx).text(),
                                            self.controls.value("mode"),
                                        )
                                    } else {
                                        "Paste text or use an example to begin.".into()
                                    }),
                            )
                        },
                    ),
            )
            .child(
                div()
                    .border_t_1()
                    .border_color(p.separator)
                    .bg(p.surface)
                    .p(px(16.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(11.))
                            .text_color(p.secondary)
                            .child(self.status.clone()),
                    )
                    .when(self.selected == "screenshot", |s| {
                        s.child(
                            button("capture-screen", "Capture Screenshot…", p)
                                .on_click(cx.listener(|this, _, _, cx| this.capture_screen(cx))),
                        )
                        .child(
                            button("ocr-image", "Mac / local OCR…", p)
                                .on_click(cx.listener(|this, _, _, cx| this.ocr_image(cx))),
                        )
                    })
                    .when(self.selected == "settings", |s| {
                        s.child(
                            button("updates", "Check for Updates…", p)
                                .on_click(cx.listener(|this, _, _, cx| this.check_updates(cx))),
                        )
                    })
                    .when(
                        !matches!(
                            self.selected.as_str(),
                            "listSet" | "stringEscape" | "subnet" | "chmod" | "numberBase"
                        ),
                        |s| {
                            s.child(button("use-input", "Use as Input", p).on_click(cx.listener(
                                |this, _, _, cx| {
                                    let text = this.output.read(cx).text().to_string();
                                    if this.busy || this.error.is_some() || text.is_empty() {
                                        return;
                                    }
                                    this.controls.reverse_after_chaining(&this.selected);
                                    this.input.update(cx, |e, cx| e.set_text(text, cx));
                                },
                            )))
                        },
                    )
                    .child(
                        button("copy-result", "Copy Result", p)
                            .when(
                                matches!(
                                    self.selected.as_str(),
                                    "stringEscape" | "subnet" | "chmod" | "numberBase"
                                ) && !crate::tool_controls::source_copy_enabled(
                                    self.busy,
                                    self.error.is_some(),
                                    self.output.read(cx).text(),
                                ),
                                |s| s.opacity(0.45).cursor_default(),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                if matches!(
                                    this.selected.as_str(),
                                    "listSet" | "stringEscape" | "subnet" | "chmod" | "numberBase"
                                ) && !crate::tool_controls::source_copy_enabled(
                                    this.busy,
                                    this.error.is_some(),
                                    this.output.read(cx).text(),
                                ) {
                                    return;
                                }
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    this.output.read(cx).text().into(),
                                ));
                                this.status = "Result copied.".into();
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
    }
    fn render_text(&self, p: crate::theme::Palette, cx: &mut Context<Self>) -> gpui::AnyElement {
        const CATEGORIES: [&str; 7] = [
            "Case", "Encode", "Decode", "Pretty", "Hash", "Lines", "Count",
        ];
        let defaults = [
            "upper", "base64", "decode", "pretty", "hash", "sort", "count",
        ];
        let options: &[(&str, &str)] = match self.text_category {
            0 => &[
                ("upper", "UPPERCASE"),
                ("lower", "lowercase"),
                ("title", "Title Case"),
                ("sentence", "Sentence case"),
                ("camel", "camelCase"),
                ("pascal", "PascalCase"),
                ("snake", "snake_case"),
                ("kebab", "kebab-case"),
                ("constant", "CONSTANT_CASE"),
            ],
            1 => &[
                ("base64", "Base64"),
                ("url", "URL"),
                ("html", "HTML entities"),
                ("hex", "Hex"),
            ],
            2 => &[
                ("decode", "Auto-detect"),
                ("decode-base64", "Base64"),
                ("decode-url", "URL"),
                ("decode-html", "HTML entities"),
                ("decode-hex", "Hex"),
            ],
            5 => &[
                ("sort", "Sort A → Z"),
                ("sort-reverse", "Sort Z → A"),
                ("reverse", "Reverse"),
                ("unique", "Remove duplicates"),
                ("nonempty", "Remove empty lines"),
                ("trim", "Trim each line"),
            ],
            _ => &[],
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(12.))
            .bg(p.bg)
            .text_color(p.primary)
            .font_family(crate::theme::ui_font())
            .child(self.header(p, true, cx))
            .child(
                div()
                    .flex()
                    .p(px(2.))
                    .gap(px(2.))
                    .rounded(px(7.))
                    .border_1()
                    .border_color(p.separator)
                    .bg(p.surface)
                    .children(CATEGORIES.into_iter().enumerate().map(|(i, label)| {
                        let selected = i == self.text_category;
                        div()
                            .id(("text-category", i))
                            .flex_1()
                            .h(px(26.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(5.))
                            .text_size(px(12.))
                            .text_color(if selected { p.accent } else { p.secondary })
                            .when(selected, |s| {
                                s.border_1().border_color(p.separator).bg(p.surface)
                            })
                            .cursor_pointer()
                            .child(label)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.text_category = i;
                                this.choose(defaults[i], cx);
                                cx.notify();
                            }))
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(self.input_actions("Input", p, cx))
                    .child(editor_card(self.input.clone(), 132., p))
                    .child(link("reset-input", "Reset", p).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.input
                                .update(cx, |e, cx| e.set_text(this.initial_text.clone(), cx))
                        },
                    ))),
            )
            .child(div().h(px(1.)).bg(p.separator))
            .child(
                div()
                    .id("text-content")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(div().flex().flex_wrap().gap(px(8.)).children(
                        options.iter().enumerate().map(|(i, (value, label))| {
                            let value = *value;
                            let selected = self.second.read(cx).text() == value;
                            button(("text-option", i), *label, p)
                                .w(px(150.))
                                .h(px(34.))
                                .justify_start()
                                .text_color(if selected { p.accent } else { p.secondary })
                                .bg(if selected { p.surface } else { p.well })
                                .on_click(cx.listener(move |this, _, _, cx| this.choose(value, cx)))
                        }),
                    ))
                    .child(editor_card(self.output.clone(), 210., p)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(11.))
                            .text_color(p.secondary)
                            .child(self.error.clone().unwrap_or_else(|| self.status.clone())),
                    )
                    .child(button("text-copy", "Copy Result", p).on_click(cx.listener(
                        |this, _, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(
                                this.output.read(cx).text().into(),
                            ));
                            this.status = "Result copied.".into();
                            cx.notify();
                        },
                    ))),
            )
            .into_any_element()
    }
    fn render_ai(&self, p: crate::theme::Palette, cx: &mut Context<Self>) -> gpui::AnyElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(12.))
            .bg(p.bg)
            .text_color(p.primary)
            .font_family(crate::theme::ui_font())
            .child(self.header(p, true, cx))
            .child(
                div()
                    .id("ai-content")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(format!(
                                "Selected text · {} characters",
                                self.input.read(cx).text().chars().count()
                            )),
                    )
                    .child(editor_card(self.input.clone(), 80., p))
                    .when(!crate::transport::provider_is_configured(), |s| {
                        s.child(
                            div()
                                .p(px(8.))
                                .rounded(px(8.))
                                .bg(p.accent.opacity(0.08))
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .text_size(px(11.))
                                .child("No AI provider configured.")
                                .child(div().flex_1())
                                .child(
                                    button("provider-settings", "Open Settings", p).on_click(
                                        |_, _, cx| open_tool("settings", String::new(), cx),
                                    ),
                                ),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .flex_1()
                                    .child(editor_card(self.second.clone(), 42., p)),
                            )
                            .child(
                                button("ai-send", "Send", p)
                                    .bg(p.accent_fill)
                                    .text_color(rgb(0xffffff))
                                    .on_click(cx.listener(|this, _, _, cx| this.send_ai(cx))),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Writing actions"),
                    )
                    .child(div().flex().flex_wrap().gap(px(8.)).children(
                        bellobox_core::ai::QUICK_ACTIONS.iter().enumerate().map(
                            |(i, (label, instruction))| {
                                let instruction = *instruction;
                                button(("writing-action", i), *label, p)
                                    .w(px(210.))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.second
                                            .update(cx, |e, cx| e.set_text(instruction.into(), cx));
                                        this.send_ai(cx);
                                    }))
                            },
                        ),
                    ))
                    .child(editor_card(self.output.clone(), 240., p)),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(p.secondary)
                    .child(self.status.clone()),
            )
            .into_any_element()
    }
}
fn option_label(id: &str) -> &'static str {
    match id {
        "regex" => "Regular expression",
        "jsonPointer" => "Pointer · empty = root",
        "jsonRedact" => "Field names (comma separated)",
        "jsonCode" => "Root type",
        "sqlInsert" => "Table name",
        "contrast" => "Background",
        "gradient" => "End color",
        "hmac" => "Key · stays in memory",
        "time" | "cron" => "Time zone",
        "snippets" => "Fill template fields",
        "units" => "To",
        "numberBase" => "Input base",
        "dateMath" => "Date operation",
        "csvExplore" => "Filter rows and columns",
        _ => "Options",
    }
}
impl Render for BelloBox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let start = Instant::now();
        let p = crate::theme::for_window(window);
        if self.input.read(cx).appearance().text != p.primary {
            for editor in [&self.input, &self.second, &self.output] {
                editor.update(cx, |e, cx| {
                    let mut style = e.appearance().clone();
                    style.text = p.primary;
                    style.caret = p.accent;
                    style.selection = p.accent.opacity(0.18);
                    e.set_appearance(style, cx);
                });
            }
        }
        self.snippet_appearance(p, cx);
        self.permission_appearance(p, cx);
        self.sync_snippet_fields(window, cx);
        let content = match self.selected.as_str() {
            "qr" => self.render_qr(p, cx),
            "textTools" => self.render_text(p, cx),
            "ai" => self.render_ai(p, cx),
            "settings" | "setup" | "updates" => div()
                .size_full()
                .p(px(24.))
                .flex()
                .flex_col()
                .gap(px(16.))
                .bg(p.bg)
                .text_color(p.primary)
                .font_family(crate::theme::ui_font())
                .child(
                    div()
                        .text_size(px(24.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(match self.selected.as_str() {
                            "setup" => "Setup guide",
                            "updates" => "Check for updates",
                            _ => "Settings",
                        }),
                )
                .child(div().text_size(px(13.)).child(self.status.clone()))
                .child(div().flex_1())
                .child(
                    button("close-status", "Close", p)
                        .on_click(|_, window, cx| crate::shutdown::close_window(window, cx)),
                )
                .into_any_element(),
            _ => self.render_workbench(p, window, cx),
        };
        let view = div()
            .size_full()
            .capture_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                if this.permission_tab(event, window, cx) {
                    return;
                }
                let Some(id) = this.open_menu else {
                    return;
                };
                if id == "snippets-library" {
                    this.snippet_menu_key(&event.keystroke.key, window, cx);
                    cx.stop_propagation();
                    return;
                }
                if let Some(spec) = crate::tool_controls::choices(&this.selected)
                    .into_iter()
                    .find(|s| s.id == id)
                {
                    match event.keystroke.key.as_str() {
                        "escape" => this.open_menu = None,
                        "up" => this.menu_index = this.menu_index.saturating_sub(1),
                        "down" => {
                            this.menu_index = (this.menu_index + 1).min(spec.choices.len() - 1)
                        }
                        "enter" => {
                            this.controls
                                .select(&this.selected, id, spec.choices[this.menu_index]);
                            this.open_menu = None;
                            this.run_tool(cx);
                        }
                        _ => {}
                    }
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                let command = event.keystroke.modifiers.platform
                    || cfg!(target_os = "linux") && event.keystroke.modifiers.control;
                if command {
                    match event.keystroke.key.as_str() {
                        "w" => crate::shutdown::close_window(window, cx),
                        "n" => open_tool(&this.selected, String::new(), cx),
                        "k" => open_launcher(this.input.read(cx).text().into(), cx),
                        _ => return,
                    }
                    cx.stop_propagation();
                }
            }))
            .child(content);
        perf("render_cpu", start.elapsed().as_micros());
        view
    }
}
pub fn open_tool(id: &str, input: String, cx: &mut App) {
    open_tool_with_clock_context(id, input, None, cx);
}
/// Explicit launcher adoption is distinct from an ordinary repeat-open.
pub fn open_clock_handoff(
    input: String,
    handoff: crate::clock_preview_session::ClockHandoff,
    cx: &mut App,
) -> Result<(), String> {
    if crate::shutdown::requested(cx) {
        return Err("The app is closing.".into());
    }
    crate::screenshot_ui::area_navigation_changed(cx);
    crate::world_clock_ui::open_with_handoff(input, Some(handoff), cx)
}
fn open_tool_with_clock_context(
    id: &str,
    input: String,
    clock_handoff: Option<crate::clock_preview_session::ClockHandoff>,
    cx: &mut App,
) {
    if crate::shutdown::requested(cx) {
        return;
    }
    crate::screenshot_ui::area_navigation_changed(cx);
    if id == "settings" {
        crate::settings_ui::open(cx);
        return;
    }
    if bellobox_core::validate_input(&input).is_ok()
        && launcher::catalog().iter().any(|c| c.id == id)
        && let Ok(mut settings) = Settings::load(&config_dir().join("settings.json"))
    {
        settings.explicit_open(id, launcher::category(&input), now());
        let _ = settings.save(&config_dir().join("settings.json"));
    }

    if id == "worldClock" {
        if let Err(error) = crate::world_clock_ui::open_with_handoff(input, clock_handoff, cx) {
            eprintln!("Cannot open World Clock: {error}");
        }
        return;
    }

    if id == "recording" {
        crate::recording_ui::open(cx);
        return;
    }

    if id == "videoToGIF" {
        crate::gif_converter::open(cx);
        return;
    }

    if id == "screenshot" {
        crate::screenshot_ui::open(cx);
        return;
    }

    let (width, height, min_w, min_h) = match id {
        "qr" => (520., 620., 400., 480.),
        "textTools" => (720., 660., 580., 520.),
        "ai" => (720., 600., 580., 480.),
        _ => (820., 660., 740., 560.),
    };
    let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
    let id = id.to_string();
    let title = format!("{} — Bello Box", crate::theme::tool_title(&id));
    if let Err(error) = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(min_w), px(min_h))),
            titlebar: Some(TitlebarOptions {
                title: Some(title.into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        move |window, cx| {
            crate::shutdown::guard_window(window, cx);
            cx.new(|cx| BelloBox::new_for(id, input, window, cx))
        },
    ) {
        eprintln!("Cannot open tool window: {error}");
    }
}
pub fn open_launcher(input: String, cx: &mut App) {
    if crate::shutdown::requested(cx) {
        return;
    }
    crate::screenshot_ui::area_navigation_changed(cx);
    crate::launcher_ui::open(input, cx);
}
pub fn run() {
    let start = Instant::now();
    Application::new().run(move |cx: &mut App| {
        bello_workbench_ui::init(cx);
        if let Ok(settings) = Settings::load(&config_dir().join("settings.json")) {
            crate::theme::set_saved_appearance(settings.appearance);
        }
        crate::shutdown::init(cx);
        if let Ok(tool) = std::env::var("BELLOBOX_TOOL")
            && tool != "home"
        {
            open_tool(
                &tool,
                std::env::var("BELLOBOX_INPUT").unwrap_or_default(),
                cx,
            );
            cx.activate(true);
            return;
        }
        let bounds = Bounds::centered(None, size(px(1000.), px(760.)), cx);
        let result = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(900.), px(640.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Bello Box".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                crate::shutdown::guard_window(window, cx);
                cx.new(|cx| crate::home::Home::new(window, cx))
            },
        );
        match result {
            Ok(_) => {
                perf("window_created", start.elapsed().as_micros());
                cx.activate(true);
            }
            Err(e) => {
                eprintln!("Cannot open Bello Box: {e}");
                crate::shutdown::request_quit(cx);
            }
        }
    });
}

#[cfg(test)]
mod quit_tests;

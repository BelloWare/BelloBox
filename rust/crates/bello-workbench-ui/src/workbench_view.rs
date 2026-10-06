use crate::{EditorEvent, EditorView, WorkbenchAppearance};
use bello_workbench::{
    diff::{DiffLineKind, ParsedDiff},
    document::{FileDocument, OpenedFile},
    git::{
        CancellationToken, Commit, DiffRequest, GitReadOptions, GitRepository, RepositoryStatus,
    },
    tree::{DirectoryEntry, DirectoryTree, EntryKind},
};
use gpui::{prelude::*, *};
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Range,
    path::PathBuf,
};
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WorkbenchPanel {
    Editor,
    Changes,
    History,
}
#[derive(Clone)]
struct TreeRow {
    entry: DirectoryEntry,
    depth: usize,
}
pub struct WorkbenchView {
    appearance: WorkbenchAppearance,
    root: PathBuf,
    editor: Entity<EditorView>,
    _subscription: Subscription,
    directories: BTreeMap<PathBuf, Vec<DirectoryEntry>>,
    expanded: BTreeSet<PathBuf>,
    tree_rows: Vec<TreeRow>,
    tree_loading: BTreeSet<PathBuf>,
    tree_serial: BTreeMap<PathBuf, u64>,
    repository: Option<GitRepository>,
    status: Option<RepositoryStatus>,
    commits: Vec<Commit>,
    next_history: Option<usize>,
    diff: Option<ParsedDiff>,
    diff_title: String,
    tab: WorkbenchPanel,
    document: Option<FileDocument>,
    file_label: String,
    notice: String,
    loading_file: bool,
    saving: bool,
    pending_open: Option<PathBuf>,
    file_generation: u64,
    git_generation: u64,
    git_cancel: CancellationToken,
    git_loading: bool,
}
impl WorkbenchView {
    pub fn new(root: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let appearance = match window.appearance() {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => WorkbenchAppearance::dark(),
            _ => WorkbenchAppearance::light(),
        };
        let editor = cx.new(|cx| {
            let mut editor = EditorView::new(
                "Choose a file from the folder tree.\n\nFiles are read in the background. UTF-8 files up to 8 MiB are editable.\nLarger text files open as an explicitly read-only preview.\n".into(), window, cx);
            editor.set_appearance(appearance.editor.clone(), cx);
            editor.set_read_only(true, cx);
            editor
        });
        let subscription = cx.subscribe(&editor, |this, _, event, cx| match event {
            EditorEvent::Changed | EditorEvent::LayoutChanged => cx.notify(),
            EditorEvent::SaveRequested => this.save(cx),
        });
        let mut s = Self {
            appearance,
            root,
            editor,
            _subscription: subscription,
            directories: BTreeMap::new(),
            expanded: BTreeSet::from([PathBuf::new()]),
            tree_rows: Vec::new(),
            tree_loading: BTreeSet::new(),
            tree_serial: BTreeMap::new(),
            repository: None,
            status: None,
            commits: Vec::new(),
            next_history: None,
            diff: None,
            diff_title: String::new(),
            tab: WorkbenchPanel::Editor,
            document: None,
            file_label: "File editor".into(),
            notice: String::new(),
            loading_file: false,
            saving: false,
            pending_open: None,
            file_generation: 0,
            git_generation: 0,
            git_cancel: CancellationToken::new(),
            git_loading: false,
        };
        s.load_directory(PathBuf::new(), cx);
        s.discover_git(cx);
        s
    }
    pub fn set_appearance(&mut self, appearance: WorkbenchAppearance, cx: &mut Context<Self>) {
        self.editor.update(cx, |ed, cx| {
            ed.set_appearance(appearance.editor.clone(), cx)
        });
        self.appearance = appearance;
        cx.notify();
    }
    pub fn set_panel(&mut self, panel: WorkbenchPanel, cx: &mut Context<Self>) {
        self.tab = panel;
        match panel {
            WorkbenchPanel::Changes => self.refresh_status(cx),
            WorkbenchPanel::History => self.history(0, cx),
            WorkbenchPanel::Editor => cx.notify(),
        }
    }
    pub fn set_vim(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.editor.update(cx, |ed, cx| ed.set_vim(enabled, cx));
    }
    /// Whether this workbench's visible editor owns editable keyboard input.
    /// Hosts must additionally check that this workbench itself is displayed.
    /// Inspect only: do not focus, commit composition or change editor state.
    pub fn has_focused_editable_text(&self, window: &Window, cx: &App) -> bool {
        let editor = self.editor.read(cx);
        focused_editable_text(
            self.tab,
            editor.engine.read_only,
            editor.focus_handle(cx).is_focused(window),
        )
    }
    pub fn root(&self) -> &std::path::Path {
        &self.root
    }
    pub fn has_unsaved_changes(&self, cx: &App) -> bool {
        self.dirty(cx)
    }
    fn dirty(&self, cx: &App) -> bool {
        self.document
            .as_ref()
            .is_some_and(|d| d.is_dirty(self.editor.read(cx).text()))
    }
    fn rebuild_tree(&mut self) {
        fn visit(
            path: &PathBuf,
            depth: usize,
            dirs: &BTreeMap<PathBuf, Vec<DirectoryEntry>>,
            expanded: &BTreeSet<PathBuf>,
            out: &mut Vec<TreeRow>,
        ) {
            if depth > 64 || out.len() >= 50_000 {
                return;
            }
            if let Some(entries) = dirs.get(path) {
                for e in entries {
                    if out.len() >= 50_000 {
                        break;
                    }
                    out.push(TreeRow {
                        entry: e.clone(),
                        depth,
                    });
                    if expanded.contains(&e.path) {
                        visit(&e.path, depth + 1, dirs, expanded, out);
                    }
                }
            }
        }
        self.tree_rows.clear();
        visit(
            &PathBuf::new(),
            0,
            &self.directories,
            &self.expanded,
            &mut self.tree_rows,
        );
    }
    fn load_directory(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.tree_loading.contains(&path) {
            return;
        }
        let serial = self.tree_serial.entry(path.clone()).or_default();
        *serial += 1;
        let serial = *serial;
        self.tree_loading.insert(path.clone());
        let root = self.root.clone();
        let request = path.clone();
        let task = cx.background_executor().spawn(async move {
            DirectoryTree::new(root).and_then(|tree| tree.read_dir(request, 2_000))
        });
        cx.spawn(async move|this,cx|{let result=task.await;let _=this.update(cx,|this,cx|{if this.tree_serial.get(&path)!=Some(&serial){return;}this.tree_loading.remove(&path);match result{Ok(page)=>{if page.truncated{this.notice="Folder preview is limited to 2,000 entries; only the inspected subset is shown".into();}this.directories.insert(path,page.entries);this.rebuild_tree();},Err(e)=>this.notice=format!("Folder could not be read: {e}")};cx.notify();});}).detach();
    }
    fn tree_click(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = self.tree_rows.get(index).cloned() else {
            return;
        };
        if row.entry.is_expandable() {
            if !self.expanded.remove(&row.entry.path) {
                self.expanded.insert(row.entry.path.clone());
                if !self.directories.contains_key(&row.entry.path) {
                    self.load_directory(row.entry.path.clone(), cx);
                }
            }
            self.rebuild_tree();
            cx.notify();
        } else if row.entry.kind == EntryKind::File {
            self.request_open(row.entry.path, cx);
            self.editor.read(cx).focus(window);
        } else {
            self.notice = "Links and special files are not edited through the tree".into();
            cx.notify();
        }
    }
    pub fn open_file(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.request_open(path, cx);
    }
    fn request_open(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.saving {
            self.notice = "Wait for the current save to finish".into();
            cx.notify();
            return;
        }
        if self.dirty(cx) {
            self.pending_open = Some(path);
            self.notice =
                "This file has unsaved changes. Save or discard before opening another file".into();
            cx.notify();
            return;
        }
        self.begin_open(path, cx);
    }
    fn begin_open(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.file_generation += 1;
        let generation = self.file_generation;
        let editor_revision = self.editor.read(cx).engine.revision();
        self.loading_file = true;
        self.pending_open = None;
        self.tab = WorkbenchPanel::Editor;
        let root = self.root.clone();
        let task = cx
            .background_executor()
            .spawn(async move { FileDocument::open(root, path) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.file_generation != generation {
                    return;
                }
                this.loading_file = false;
                if this.editor.read(cx).engine.revision() != editor_revision {
                    this.notice = "Your draft changed while the file was opening. It has been preserved; select the file again".into();cx.notify();return;
                }
                match result {
                    Ok(OpenedFile::Editable(document)) => {
                        this.file_label = document
                            .path()
                            .strip_prefix(&this.root)
                            .unwrap_or(document.path())
                            .display()
                            .to_string();
                        this.editor.update(cx, |editor, cx| {
                            editor.set_read_only(false, cx);
                            editor.set_text(document.text().into(), cx);
                        });
                        this.document = Some(document);
                        this.notice.clear();
                    }
                    Ok(OpenedFile::Preview(preview)) => {
                        this.file_label = preview.path.display().to_string();
                        this.editor.update(cx, |editor, cx| {
                            editor.set_text(preview.text, cx);
                            editor.set_read_only(true, cx);
                        });
                        this.document = None;
                        this.notice = preview.reason;
                    }
                    Err(e) => this.notice = e.to_string(),
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn save(&mut self, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let Some(mut document) = self.document.clone() else {
            self.notice = "Choose an editable file before saving".into();
            cx.notify();
            return;
        };
        let text = self.editor.read(cx).text().to_owned();
        if !document.is_dirty(&text) {
            if let Some(path) = self.pending_open.take() {
                self.begin_open(path, cx);
            }
            return;
        }
        self.saving = true;
        let generation = self.file_generation;
        let task = cx
            .background_executor()
            .spawn(async move { document.save(&text).map(|outcome| (document, outcome)) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.saving = false;
                if this.file_generation != generation {
                    return;
                }
                match result {
                    Ok((document, outcome)) => {
                        this.document = Some(document);
                        this.notice = if outcome.directory_synced {
                            "Saved".into()
                        } else {
                            "Saved; directory durability could not be confirmed".into()
                        };
                        this.refresh_status(cx);
                        if !this.dirty(cx)
                            && let Some(path) = this.pending_open.take()
                        {
                            this.begin_open(path, cx);
                        }
                    }
                    Err(e) => this.notice = e.to_string(),
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn discover_git(&mut self, cx: &mut Context<Self>) {
        let root = self.root.clone();
        let task = cx
            .background_executor()
            .spawn(async move { GitRepository::open(root) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(repo) => {
                    this.repository = Some(repo);
                    this.refresh_status(cx);
                }
                Err(e) => {
                    this.notice = format!("Git unavailable for this folder: {e}");
                    cx.notify();
                }
            });
        })
        .detach();
    }
    fn git_options(&mut self) -> (u64, GitReadOptions) {
        self.git_cancel.cancel();
        self.git_cancel = CancellationToken::new();
        self.git_generation += 1;
        self.git_loading = true;
        (
            self.git_generation,
            GitReadOptions {
                cancellation: self.git_cancel.clone(),
                ..Default::default()
            },
        )
    }
    fn refresh_status(&mut self, cx: &mut Context<Self>) {
        let Some(repo) = self.repository.clone() else {
            return;
        };
        let (generation, options) = self.git_options();
        let task = cx
            .background_executor()
            .spawn(async move { repo.status(&options) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.git_generation != generation {
                    return;
                }
                this.git_loading = false;
                match result {
                    Ok(status) => this.status = Some(status),
                    Err(e) => this.notice = e.to_string(),
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn history(&mut self, skip: usize, cx: &mut Context<Self>) {
        let Some(repo) = self.repository.clone() else {
            return;
        };
        self.tab = WorkbenchPanel::History;
        let (generation, options) = self.git_options();
        let task = cx
            .background_executor()
            .spawn(async move { repo.history(skip, 100, &options) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.git_generation != generation {
                    return;
                }
                this.git_loading = false;
                match result {
                    Ok(page) => {
                        if skip == 0 {
                            this.commits.clear();
                        }
                        this.commits.extend(page.commits);
                        this.next_history = page.next_skip;
                    }
                    Err(e) => this.notice = e.to_string(),
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn show_diff(&mut self, request: DiffRequest, title: String, cx: &mut Context<Self>) {
        let Some(repo) = self.repository.clone() else {
            return;
        };
        let (generation, options) = self.git_options();
        self.diff_title = title;
        self.diff = None;
        let task = cx.background_executor().spawn(async move {
            repo.diff(&request, &options)
                .map(|d| ParsedDiff::parse(&d.lossy_text(), 20_000))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.git_generation != generation {
                    return;
                }
                this.git_loading = false;
                match result {
                    Ok(diff) => {
                        if diff.truncated {
                            this.notice = "Diff preview stopped at 20,000 lines".into();
                        } else {
                            this.notice.clear();
                        }
                        this.diff = Some(diff);
                    }
                    Err(e) => this.notice = e.to_string(),
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn change_diff(&mut self, index: usize, staged: bool, cx: &mut Context<Self>) {
        let Some(entry) = self
            .status
            .as_ref()
            .and_then(|s| s.entries.get(index))
            .cloned()
        else {
            return;
        };
        if entry.untracked {
            self.request_open(
                self.repository.as_ref().unwrap().root().join(&entry.path),
                cx,
            );
            return;
        }
        let mut paths = vec![entry.path.clone()];
        if let Some(original) = entry.original_path {
            paths.push(original);
        }
        let title = format!(
            "{} · {}",
            if staged { "Staged" } else { "Worktree" },
            entry.path.display()
        );
        self.show_diff(
            if staged {
                DiffRequest::Staged { paths }
            } else {
                DiffRequest::Worktree { paths }
            },
            title,
            cx,
        );
    }
    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let dirty = self.dirty(cx);
        div()
            .h(px(self.appearance.toolbar_height))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .bg(self.appearance.surface)
            .border_b_1()
            .border_color(self.appearance.border)
            .children(
                [
                    (WorkbenchPanel::Editor, "Editor"),
                    (WorkbenchPanel::Changes, "Changes"),
                    (WorkbenchPanel::History, "History"),
                ]
                .into_iter()
                .map(|(tab, label)| {
                    div()
                        .id(label)
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .cursor_pointer()
                        .when(self.tab == tab, |d| {
                            d.bg(self.appearance.selected)
                                .text_color(self.appearance.accent)
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.tab = tab;
                            match tab {
                                WorkbenchPanel::Changes => this.refresh_status(cx),
                                WorkbenchPanel::History if this.commits.is_empty() => {
                                    this.history(0, cx)
                                }
                                _ => cx.notify(),
                            }
                        }))
                        .child(label)
                }),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id("save-file")
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .text_color(self.appearance.accent)
                    .on_click(cx.listener(|this, _, _, cx| this.save(cx)))
                    .child(if self.saving {
                        "Saving…"
                    } else if dirty {
                        "Save •"
                    } else {
                        "Save"
                    }),
            )
    }
    fn render_diff(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(diff) = &self.diff else {
            return div()
                .p_5()
                .text_color(self.appearance.muted)
                .child(if self.git_loading {
                    "Reading Git…"
                } else {
                    "Select a changed file or commit to view its patch"
                })
                .into_any_element();
        };
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(
                div()
                    .h(px(32.))
                    .px_3()
                    .py_1()
                    .text_size(px(12.))
                    .bg(self.appearance.surface)
                    .child(format!(
                        "{}    +{} −{}{}",
                        self.diff_title,
                        diff.added,
                        diff.removed,
                        if diff.truncated {
                            " · preview truncated"
                        } else {
                            ""
                        }
                    )),
            )
            .child(
                uniform_list(
                    "git-diff-lines",
                    diff.lines.len(),
                    cx.processor(|this, range: Range<usize>, _, _| {
                        range
                            .map(|i| {
                                let line = &this.diff.as_ref().unwrap().lines[i];
                                let (color, bg) = match line.kind {
                                    DiffLineKind::Added => (
                                        this.appearance.added_text,
                                        this.appearance.added_background,
                                    ),
                                    DiffLineKind::Removed => (
                                        this.appearance.removed_text,
                                        this.appearance.removed_background,
                                    ),
                                    DiffLineKind::Hunk => {
                                        (this.appearance.hunk_text, this.appearance.hunk_background)
                                    }
                                    _ => (this.appearance.text, this.appearance.background),
                                };
                                let text = line.text.chars().take(8_192).collect::<String>();
                                div()
                                    .h(px(22.))
                                    .flex()
                                    .bg(bg)
                                    .text_color(color)
                                    .font_family("monospace")
                                    .text_size(px(12.))
                                    .child(
                                        div()
                                            .w(px(100.))
                                            .flex_shrink_0()
                                            .text_color(this.appearance.muted)
                                            .child(format!(
                                                "{:>5} {:>5}",
                                                line.old_line
                                                    .map(|n| n.to_string())
                                                    .unwrap_or_default(),
                                                line.new_line
                                                    .map(|n| n.to_string())
                                                    .unwrap_or_default()
                                            )),
                                    )
                                    .child(text)
                            })
                            .collect()
                    }),
                )
                .flex_1()
                .min_h_0(),
            )
            .into_any_element()
    }
}
impl Drop for WorkbenchView {
    fn drop(&mut self) {
        self.git_cancel.cancel();
    }
}
impl Render for WorkbenchView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tree = div()
            .w(px(self.appearance.tree_width))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(self.appearance.border)
            .bg(self.appearance.sidebar)
            .child(
                div()
                    .h(px(self.appearance.toolbar_height))
                    .px_3()
                    .flex()
                    .items_center()
                    .text_size(px(12.))
                    .text_color(self.appearance.text)
                    .child(
                        self.root
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                    ),
            )
            .child(
                uniform_list(
                    "folder-tree",
                    self.tree_rows.len(),
                    cx.processor(|this, range: Range<usize>, _, cx| {
                        range
                            .map(|i| {
                                let row = &this.tree_rows[i];
                                let glyph = match row.entry.kind {
                                    EntryKind::Directory => {
                                        if this.expanded.contains(&row.entry.path) {
                                            "▾"
                                        } else {
                                            "▸"
                                        }
                                    }
                                    EntryKind::File => "·",
                                    EntryKind::Symlink => "↗",
                                    EntryKind::Other => "?",
                                };
                                div()
                                    .id(i)
                                    .h(px(this.appearance.tree_row_height))
                                    .pl(px(12. + row.depth as f32 * 14.))
                                    .pr_2()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .cursor_pointer()
                                    .hover(|d| d.bg(this.appearance.hover))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.tree_click(i, window, cx)
                                    }))
                                    .child(
                                        div()
                                            .w(px(12.))
                                            .text_color(this.appearance.accent)
                                            .child(glyph),
                                    )
                                    .child(row.entry.name.to_string_lossy().to_string())
                            })
                            .collect()
                    }),
                )
                .flex_1()
                .min_h_0(),
            )
            .child(
                div()
                    .h(px(30.))
                    .px_3()
                    .text_size(px(11.))
                    .text_color(self.appearance.muted)
                    .child(if self.tree_loading.is_empty() {
                        "Lazy folder tree"
                    } else {
                        "Loading folder…"
                    }),
            );
        let main = match self.tab {
            WorkbenchPanel::Editor => div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .child(
                    div()
                        .h(px(32.))
                        .px_3()
                        .flex()
                        .items_center()
                        .text_size(px(12.))
                        .child(format!(
                            "{}{}{}",
                            self.file_label,
                            if self.dirty(cx) { " •" } else { "" },
                            if self.loading_file {
                                " · opening…"
                            } else {
                                ""
                            }
                        )),
                )
                .child(self.editor.clone())
                .into_any_element(),
            WorkbenchPanel::Changes => {
                let count = self.status.as_ref().map(|s| s.entries.len()).unwrap_or(0);
                let branch = self
                    .status
                    .as_ref()
                    .and_then(|s| s.branch.clone())
                    .unwrap_or_else(|| "No repository".into());
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .h(px(30.))
                            .px_3()
                            .child(format!("{branch} · {count} changed paths")),
                    )
                    .child(
                        uniform_list(
                            "git-changes",
                            count,
                            cx.processor(|this, range: Range<usize>, _, cx| {
                                range
                                    .map(|i| {
                                        let e = &this.status.as_ref().unwrap().entries[i];
                                        div()
                                            .id(i)
                                            .h(px(30.))
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .px_3()
                                            .child(
                                                div()
                                                    .w(px(40.))
                                                    .text_color(this.appearance.accent)
                                                    .child(format!(
                                                        "{}{}",
                                                        e.index_status, e.worktree_status
                                                    )),
                                            )
                                            .child(
                                                div().flex_1().child(e.path.display().to_string()),
                                            )
                                            .when(e.staged(), |d| {
                                                d.child(
                                                    div()
                                                        .id(("staged", i))
                                                        .px_2()
                                                        .cursor_pointer()
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.change_diff(i, true, cx)
                                                            },
                                                        ))
                                                        .child("Staged"),
                                                )
                                            })
                                            .when(e.unstaged(), |d| {
                                                d.child(
                                                    div()
                                                        .id(("worktree", i))
                                                        .px_2()
                                                        .cursor_pointer()
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.change_diff(i, false, cx)
                                                            },
                                                        ))
                                                        .child(if e.untracked {
                                                            "Open"
                                                        } else {
                                                            "Worktree"
                                                        }),
                                                )
                                            })
                                    })
                                    .collect()
                            }),
                        )
                        .h(px(180.))
                        .flex_shrink_0(),
                    )
                    .child(self.render_diff(cx))
                    .into_any_element()
            }
            WorkbenchPanel::History => div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .child(
                    uniform_list(
                        "git-history",
                        self.commits.len(),
                        cx.processor(|this, range: Range<usize>, _, cx| {
                            range
                                .map(|i| {
                                    let c = &this.commits[i];
                                    div()
                                        .id(i)
                                        .h(px(30.))
                                        .flex()
                                        .items_center()
                                        .gap_3()
                                        .px_3()
                                        .cursor_pointer()
                                        .hover(|d| d.bg(this.appearance.hover))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            let c = this.commits[i].clone();
                                            this.show_diff(
                                                DiffRequest::Commit {
                                                    hash: c.hash,
                                                    paths: Vec::new(),
                                                },
                                                format!("{} {}", c.short_hash, c.subject),
                                                cx,
                                            );
                                        }))
                                        .child(
                                            div()
                                                .w(px(72.))
                                                .text_color(this.appearance.accent)
                                                .child(c.short_hash.clone()),
                                        )
                                        .child(div().flex_1().child(c.subject.clone()))
                                        .child(
                                            div()
                                                .text_color(this.appearance.muted)
                                                .child(c.author.clone()),
                                        )
                                })
                                .collect()
                        }),
                    )
                    .h(px(210.))
                    .flex_shrink_0(),
                )
                .when(self.next_history.is_some(), |d| {
                    d.child(
                        div()
                            .id("more-history")
                            .h(px(26.))
                            .px_3()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(skip) = this.next_history {
                                    this.history(skip, cx);
                                }
                            }))
                            .child("Load next 100 commits"),
                    )
                })
                .child(self.render_diff(cx))
                .into_any_element(),
        };
        div()
            .size_full()
            .flex()
            .bg(self.appearance.background)
            .text_color(self.appearance.text)
            .font_family(self.appearance.font_family.clone())
            .text_size(px(self.appearance.font_size))
            .when(self.appearance.show_tree, |d| d.child(tree))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .when(self.appearance.show_toolbar, |d| d.child(self.toolbar(cx)))
                    .child(main)
                    .when(!self.notice.is_empty(), |d| {
                        d.child(
                            div()
                                .px_3()
                                .py_2()
                                .text_size(px(12.))
                                .bg(self.appearance.warning_background)
                                .text_color(self.appearance.warning_text)
                                .child(self.notice.clone()),
                        )
                    })
                    .when(self.pending_open.is_some(), |d| {
                        d.child(
                            div()
                                .flex()
                                .gap_3()
                                .px_3()
                                .py_2()
                                .child(
                                    div()
                                        .id("save-then-open")
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| this.save(cx)))
                                        .child("Save and open"),
                                )
                                .child(
                                    div()
                                        .id("discard-then-open")
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            if let Some(path) = this.pending_open.take() {
                                                this.begin_open(path, cx);
                                            }
                                        }))
                                        .child("Discard and open"),
                                )
                                .child(
                                    div()
                                        .id("keep-editing")
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.pending_open = None;
                                            this.notice.clear();
                                            cx.notify();
                                        }))
                                        .child("Keep editing"),
                                ),
                        )
                    }),
            )
    }
}

fn focused_editable_text(panel: WorkbenchPanel, read_only: bool, focused: bool) -> bool {
    panel == WorkbenchPanel::Editor && !read_only && focused
}

#[cfg(test)]
mod focused_text_tests {
    use super::{WorkbenchPanel, focused_editable_text};

    #[test]
    fn only_visible_focused_editable_editor_owns_text_input() {
        for panel in [
            WorkbenchPanel::Editor,
            WorkbenchPanel::Changes,
            WorkbenchPanel::History,
        ] {
            for read_only in [false, true] {
                for focused in [false, true] {
                    assert_eq!(
                        focused_editable_text(panel, read_only, focused),
                        panel == WorkbenchPanel::Editor && !read_only && focused
                    );
                }
            }
        }
    }
}

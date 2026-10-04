//! Host-owned appearance tokens. No app model or palette is imported here.
use gpui::{Hsla, SharedString, rgb, rgba};
#[derive(Clone, Debug)]
pub struct EditorAppearance {
    pub font_family: SharedString,
    pub font_size: f32,
    pub line_height: f32,
    pub gutter_font_size: f32,
    pub gutter_width: f32,
    pub padding_x: f32,
    pub padding_y: f32,
    pub background: Option<Hsla>,
    pub text: Hsla,
    pub line_number: Hsla,
    pub selection: Hsla,
    pub caret: Hsla,
    pub normal_caret: Hsla,
    pub active_line: Option<Hsla>,
    pub status_background: Hsla,
    pub status_text: Hsla,
    pub accent: Hsla,
    pub wrap_lines: bool,
    pub show_gutter: bool,
    pub show_status: bool,
    pub show_vim_toggle: bool,
}
impl EditorAppearance {
    /// Literal multiline input, matching a plain native text-control role.
    /// Hosts supply their current light/dark palette and exact font metrics.
    pub fn plain() -> Self {
        Self {
            font_family: ".SystemUIFont".into(),
            font_size: 13.,
            line_height: 19.,
            gutter_font_size: 11.,
            gutter_width: 48.,
            padding_x: 2.,
            padding_y: 4.,
            background: None,
            text: rgb(0x1f1b17).into(),
            line_number: rgb(0x9b9288).into(),
            selection: rgba(0x98470933).into(),
            caret: rgb(0x984709).into(),
            normal_caret: rgba(0x98470966).into(),
            active_line: None,
            status_background: rgb(0xf6f1ea).into(),
            status_text: rgb(0x6f675e).into(),
            accent: rgb(0x984709).into(),
            wrap_lines: true,
            show_gutter: false,
            show_status: false,
            show_vim_toggle: false,
        }
    }
    /// File-text metrics mirror the existing shared native viewer: 12 pt mono,
    /// 11 pt line numbers, 17 pt line height. Extra status chrome is host opt-in.
    pub fn code() -> Self {
        Self {
            font_family: "monospace".into(),
            font_size: 12.,
            line_height: 17.,
            padding_x: 10.,
            padding_y: 8.,
            wrap_lines: false,
            show_gutter: true,
            ..Self::plain()
        }
    }
    pub fn dark(mut self) -> Self {
        self.text = rgb(0xf1ece5).into();
        self.line_number = rgb(0xaaa096).into();
        self.selection = rgba(0xd6752045).into();
        self.caret = rgb(0xf2b777).into();
        self.normal_caret = rgba(0xf2b77766).into();
        self.status_background = rgb(0x26221e).into();
        self.status_text = rgb(0xc3b8aa).into();
        self.accent = rgb(0xf2b777).into();
        self
    }
    pub(crate) fn validate(&mut self) {
        self.font_size = self.font_size.clamp(8., 48.);
        self.line_height = self.line_height.max(self.font_size).min(72.);
        self.gutter_font_size = self.gutter_font_size.clamp(8., 48.);
        self.gutter_width = self.gutter_width.clamp(0., 160.);
        self.padding_x = self.padding_x.clamp(0., 64.);
        self.padding_y = self.padding_y.clamp(0., 64.);
    }
}
impl Default for EditorAppearance {
    fn default() -> Self {
        Self::code()
    }
}
#[derive(Clone, Debug)]
pub struct WorkbenchAppearance {
    pub background: Hsla,
    pub surface: Hsla,
    pub sidebar: Hsla,
    pub border: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub accent: Hsla,
    pub selected: Hsla,
    pub hover: Hsla,
    pub warning_background: Hsla,
    pub warning_text: Hsla,
    pub added_background: Hsla,
    pub added_text: Hsla,
    pub removed_background: Hsla,
    pub removed_text: Hsla,
    pub hunk_background: Hsla,
    pub hunk_text: Hsla,
    pub font_family: SharedString,
    pub font_size: f32,
    pub tree_width: f32,
    pub tree_row_height: f32,
    pub toolbar_height: f32,
    pub editor: EditorAppearance,
    pub show_tree: bool,
    pub show_toolbar: bool,
}
impl WorkbenchAppearance {
    pub fn light() -> Self {
        Self {
            background: rgb(0xfcf9f5).into(),
            surface: rgb(0xffffff).into(),
            sidebar: rgb(0xf6f1ea).into(),
            border: rgb(0xe2d9ce).into(),
            text: rgb(0x1f1b17).into(),
            muted: rgb(0x6f675e).into(),
            accent: rgb(0x984709).into(),
            selected: rgb(0xf3e3d0).into(),
            hover: rgb(0xeee6dc).into(),
            warning_background: rgb(0xfff1d6).into(),
            warning_text: rgb(0x795511).into(),
            added_background: rgb(0xe8f2e6).into(),
            added_text: rgb(0x225a2a).into(),
            removed_background: rgb(0xf9e8e5).into(),
            removed_text: rgb(0x8f3330).into(),
            hunk_background: rgb(0xe8edf4).into(),
            hunk_text: rgb(0x375675).into(),
            font_family: ".SystemUIFont".into(),
            font_size: 13.,
            tree_width: 232.,
            tree_row_height: 27.,
            toolbar_height: 40.,
            editor: EditorAppearance::code(),
            show_tree: true,
            show_toolbar: true,
        }
    }
    pub fn dark() -> Self {
        Self {
            background: rgb(0x26221e).into(),
            surface: rgb(0x2e2925).into(),
            sidebar: rgb(0x1e1b18).into(),
            border: rgb(0x494037).into(),
            text: rgb(0xf1ece5).into(),
            muted: rgb(0xc3b8aa).into(),
            accent: rgb(0xf2b777).into(),
            selected: rgb(0x493623).into(),
            hover: rgb(0x3a332c).into(),
            warning_background: rgb(0x393124).into(),
            warning_text: rgb(0xf1cf9f).into(),
            added_background: rgb(0x21352b).into(),
            added_text: rgb(0xa4d3a2).into(),
            removed_background: rgb(0x392727).into(),
            removed_text: rgb(0xe8aba8).into(),
            hunk_background: rgb(0x26303c).into(),
            hunk_text: rgb(0xa2bfd4).into(),
            editor: EditorAppearance::code().dark(),
            ..Self::light()
        }
    }
}
impl Default for WorkbenchAppearance {
    fn default() -> Self {
        Self::light()
    }
}

#[cfg(test)]
mod tests {
    use super::{EditorAppearance, WorkbenchAppearance};
    #[test]
    fn plain_controls_do_not_add_code_editor_chrome() {
        let style = EditorAppearance::plain();
        assert!(!style.show_gutter && !style.show_status && !style.show_vim_toggle);
        assert!(style.background.is_none());
        assert_eq!(
            (style.font_size, style.padding_x, style.padding_y),
            (13., 2., 4.)
        );
    }
    #[test]
    fn code_viewer_keeps_native_source_metrics() {
        let style = EditorAppearance::code();
        assert_eq!(
            (style.font_size, style.gutter_font_size, style.line_height),
            (12., 11., 17.)
        );
        assert!(style.show_gutter);
        assert!(!style.show_status);
    }
    #[test]
    fn host_themes_do_not_share_light_ink_in_dark_surfaces() {
        let light = WorkbenchAppearance::light();
        let dark = WorkbenchAppearance::dark();
        assert_ne!(light.text, dark.text);
        assert_ne!(light.background, dark.background);
        assert_eq!(light.editor.font_size, dark.editor.font_size);
    }
}

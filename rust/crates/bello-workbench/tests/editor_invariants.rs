use bello_workbench::editor::{Editor, Key};

#[test]
fn randomized_modal_edits_keep_unicode_and_line_indexes_valid() {
    let mut editor = Editor::new("alpha\r\n中😀e\u{301}\nlast".into());
    editor.set_vim(true);
    let keys = [
        Key::Char('h'),
        Key::Char('j'),
        Key::Char('k'),
        Key::Char('l'),
        Key::Char('w'),
        Key::Char('b'),
        Key::Char('e'),
        Key::Char('d'),
        Key::Char('c'),
        Key::Char('y'),
        Key::Char('p'),
        Key::Char('u'),
        Key::Char('i'),
        Key::Char('a'),
        Key::Char('v'),
        Key::Char('V'),
        Key::Char('0'),
        Key::Char('$'),
        Key::Char('2'),
        Key::Char('g'),
        Key::Char('G'),
        Key::Char('x'),
        Key::Char('中'),
        Key::Escape,
        Key::Enter,
        Key::Backspace,
        Key::Delete,
        Key::Ctrl('r'),
    ];
    let mut random = 0x781b2a45u64;
    for step in 0..5000 {
        random ^= random << 13;
        random ^= random >> 7;
        random ^= random << 17;
        editor.key(keys[(random as usize) % keys.len()]);
        assert!(
            editor.cursor <= editor.text().len(),
            "cursor at step {step}"
        );
        assert!(
            editor.text().is_char_boundary(editor.cursor),
            "boundary at step {step}"
        );
        let expected_lines = 1 + editor.text().bytes().filter(|&b| b == b'\n').count();
        assert_eq!(
            editor.buffer.line_count(),
            expected_lines,
            "line index at step {step}"
        );
        for (byte, _) in editor.text().char_indices().take(32) {
            let utf16 = editor.buffer.utf16_offset(byte);
            assert_eq!(
                editor.buffer.byte_offset(utf16),
                byte,
                "UTF-16 roundtrip at step {step}"
            );
        }
    }
}

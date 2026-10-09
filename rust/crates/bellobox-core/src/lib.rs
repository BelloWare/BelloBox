//! BelloBox's portable domain layer. Constructors and previews have no external effects.
//! No clipboard history, selection, provider secrets, or HTTP response history is stored.
pub mod ai;
pub mod clock;
#[cfg(feature = "developer-tools")]
pub mod developer;
pub mod launcher;
pub mod qr;
pub mod recording;
pub mod screenshot;
pub mod settings;
pub mod snippets;
pub mod text;
pub mod url_editor;

pub const MAX_INPUT_BYTES: usize = 500_000;
pub const MAX_PREVIEW_BYTES: usize = 64_000;

pub fn validate_input(input: &str) -> Result<(), String> {
    if input.len() > MAX_INPUT_BYTES {
        Err(format!(
            "Input exceeds {MAX_INPUT_BYTES} UTF-8 bytes; it has not been truncated."
        ))
    } else {
        Ok(())
    }
}

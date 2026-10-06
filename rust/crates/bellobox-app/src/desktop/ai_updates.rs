//! Coalesce one AI polling tick without treating a quiet stream as a UI change.
use std::sync::mpsc::Receiver;

#[derive(Default)]
pub(super) struct Batch {
    pub changed: bool,
    pub done: bool,
    pub error: Option<String>,
}

pub(super) fn drain(rx: &Receiver<Result<Option<String>, String>>, text: &mut String) -> Batch {
    let mut batch = Batch::default();
    while let Ok(event) = rx.try_recv() {
        match event {
            Ok(Some(chunk)) => {
                batch.changed |= !chunk.is_empty();
                text.push_str(&chunk);
            }
            Ok(None) => batch.done = true,
            Err(error) => {
                batch.error = Some(error);
                batch.done = true;
            }
        }
    }
    batch
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    #[test]
    fn idle_and_empty_chunks_do_not_change_output_or_finish() {
        let (tx, rx) = channel();
        let mut text = "existing".to_owned();
        for tick in 0..250 {
            if tick % 2 == 0 {
                tx.send(Ok(Some(String::new()))).unwrap();
            }
            let batch = drain(&rx, &mut text);
            assert!(!batch.changed && !batch.done && batch.error.is_none());
            assert_eq!(text, "existing");
        }
    }

    #[test]
    fn coalesces_unicode_chunks_and_preserves_output_across_idle_ticks() {
        let (tx, rx) = channel();
        let mut text = String::new();
        for chunk in ["Hello ", "", "世界", " 🦀"] {
            tx.send(Ok(Some(chunk.into()))).unwrap();
        }
        let batch = drain(&rx, &mut text);
        assert!(batch.changed && !batch.done && batch.error.is_none());
        assert_eq!(text, "Hello 世界 🦀");
        let idle = drain(&rx, &mut text);
        assert!(!idle.changed && !idle.done);
        assert_eq!(text, "Hello 世界 🦀");
    }

    #[test]
    fn completion_without_new_text_still_finishes() {
        let (tx, rx) = channel();
        tx.send(Ok(None)).unwrap();
        let mut text = "previous output".into();
        let batch = drain(&rx, &mut text);
        assert!(!batch.changed && batch.done && batch.error.is_none());
        assert_eq!(text, "previous output");
    }

    #[test]
    fn final_chunks_and_completion_are_published_together() {
        let (tx, rx) = channel();
        tx.send(Ok(Some("final".into()))).unwrap();
        tx.send(Ok(None)).unwrap();
        let mut text = String::new();
        let batch = drain(&rx, &mut text);
        assert!(batch.changed && batch.done && batch.error.is_none());
        assert_eq!(text, "final");
    }

    #[test]
    fn errors_preserve_partial_output_with_or_without_new_chunks() {
        for chunk in [None, Some(" more")] {
            let (tx, rx) = channel();
            let mut text = "partial".into();
            if let Some(chunk) = chunk {
                tx.send(Ok(Some(chunk.into()))).unwrap();
            }
            tx.send(Err("offline test error".into())).unwrap();
            let batch = drain(&rx, &mut text);
            assert_eq!(batch.changed, chunk.is_some());
            assert!(batch.done);
            assert_eq!(batch.error.as_deref(), Some("offline test error"));
            assert_eq!(
                text,
                if chunk.is_some() {
                    "partial more"
                } else {
                    "partial"
                }
            );
        }
    }
}

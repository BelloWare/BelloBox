//! Window-local activation-key evidence for QR Save dialog admission.
use std::{cell::Cell, rc::Rc};

/// Window-owned physical observation, separate from preview-button ownership.
/// Recording a key never consumes text/IME input and never arms an action.
#[derive(Clone, Default)]
pub(crate) struct PhysicalActivationKeys(Rc<Cell<[bool; 2]>>);
impl PhysicalActivationKeys {
    pub fn observe(&self, key: &str, down: bool) {
        let index = match key {
            "enter" => 0,
            "space" => 1,
            _ => return,
        };
        let mut keys = self.0.get();
        keys[index] = down;
        self.0.set(keys);
    }
    pub(crate) fn any_down(&self) -> bool {
        self.0.get().into_iter().any(|down| down)
    }
}

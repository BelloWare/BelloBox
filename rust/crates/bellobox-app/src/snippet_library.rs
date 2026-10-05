//! Source: SnippetStore and UtilityWorkbenchModel's explicit library actions.
//! Only the isolated Rust store is opened; legacy Swift Snippets.json is untouched.
use bellobox_core::settings::{Snippet, Snippets};
use std::path::PathBuf;

pub(crate) struct Library {
    path: PathBuf,
    pub items: Vec<Snippet>,
    pub selected: Option<String>,
}
impl Library {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            items: Vec::new(),
            selected: None,
        }
    }
    pub fn refresh(&mut self) -> Result<(), String> {
        self.items = Snippets::load(&self.path)?.snippets;
        self.items.sort_by_key(|s| s.name.to_lowercase());
        Ok(())
    }
    pub fn filtered(&self, query: &str) -> Vec<&Snippet> {
        let query = query.to_lowercase();
        self.items
            .iter()
            .filter(|s| s.name.to_lowercase().contains(&query))
            .collect()
    }
    pub fn can_save(name: &str, body: &str) -> bool {
        !name.trim().is_empty() && !body.is_empty()
    }
    pub fn load(&mut self, id: &str) -> Result<Snippet, String> {
        self.refresh()?;
        let item = self
            .items
            .iter()
            .find(|s| s.id == id)
            .cloned()
            .ok_or("This snippet is no longer in the library.")?;
        self.selected = Some(item.id.clone());
        Ok(item)
    }
    pub fn new_draft(&mut self) {
        self.selected = None;
    }
    pub fn save(&mut self, name: &str, body: &str) -> Result<(), String> {
        if !Self::can_save(name, body) {
            return Err("Give the snippet a name and some text.".into());
        }
        self.refresh()?;
        let id = self
            .selected
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let item = Snippet {
            id: id.clone(),
            name: name.into(),
            body: body.into(),
        };
        let mut items = self.items.clone();
        if let Some(index) = items.iter().position(|s| s.id == id) {
            items[index] = item;
        } else {
            items.push(item);
        }
        self.persist(items)?;
        self.selected = Some(id);
        Ok(())
    }
    // The caller supplies the ID captured by the confirmation, never a newer selection.
    pub fn delete(&mut self, confirmed_id: &str) -> Result<(), String> {
        self.refresh()?;
        let items = self
            .items
            .iter()
            .filter(|s| s.id != confirmed_id)
            .cloned()
            .collect();
        self.persist(items)?;
        if self.selected.as_deref() == Some(confirmed_id) {
            self.new_draft();
        }
        Ok(())
    }
    fn persist(&mut self, items: Vec<Snippet>) -> Result<(), String> {
        let store = Snippets { snippets: items };
        if serde_json::to_vec_pretty(&store)
            .map_err(|e| e.to_string())?
            .len()
            > 5_000_000
        {
            return Err("Keep your snippet library under 5 MB.".into());
        }
        store.save(&self.path)?;
        self.items = store.snippets;
        self.items.sort_by_key(|s| s.name.to_lowercase());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn library() -> (tempfile::TempDir, Library) {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::new(dir.path().join("Rust/snippets.json"));
        (dir, library)
    }
    #[test]
    fn mount_and_drafts_do_not_write() {
        let (_dir, mut library) = library();
        library.refresh().unwrap();
        library.new_draft();
        assert!(!library.path.exists());
        assert!(!Library::can_save(" \n", "body"));
        assert!(!Library::can_save("name", ""));
    }
    #[test]
    fn save_updates_identity_and_new_creates_another() {
        let (_dir, mut library) = library();
        library.save("Zebra", "one").unwrap();
        let id = library.selected.clone().unwrap();
        library.save("Alpha", "two").unwrap();
        assert_eq!(library.items.len(), 1);
        assert_eq!(library.load(&id).unwrap().body, "two");
        library.new_draft();
        library.save("Beta", "three").unwrap();
        assert_ne!(library.selected.as_ref(), Some(&id));
        assert_eq!(library.filtered("ALP")[0].name, "Alpha");
        assert_eq!(library.items.len(), 2);
    }
    #[test]
    fn independent_windows_reload_before_mutating() {
        let (_dir, mut a) = library();
        let mut b = Library::new(a.path.clone());
        b.refresh().unwrap();
        a.save("A", "a").unwrap();
        b.save("B", "b").unwrap();
        a.refresh().unwrap();
        assert_eq!(a.items.len(), 2);
    }
    #[test]
    fn corrupt_store_blocks_save_and_delete_without_touching_bytes() {
        let (_dir, mut library) = library();
        library.save("A", "a").unwrap();
        let id = library.selected.clone().unwrap();
        std::fs::write(&library.path, "corrupt").unwrap();
        assert!(library.save("B", "b").is_err());
        assert!(library.delete(&id).is_err());
        assert_eq!(std::fs::read_to_string(&library.path).unwrap(), "corrupt");
        assert_eq!(library.selected.as_deref(), Some(id.as_str()));
    }
    #[test]
    fn rejected_save_keeps_persisted_and_selected_data() {
        let (_dir, mut library) = library();
        library.save("A", "a").unwrap();
        let before = std::fs::read(&library.path).unwrap();
        assert!(
            library
                .save("B", &"x".repeat(bellobox_core::MAX_INPUT_BYTES + 1))
                .is_err()
        );
        assert_eq!(std::fs::read(&library.path).unwrap(), before);
        assert_eq!(library.items[0].name, "A");
    }
    #[test]
    fn delete_targets_confirmed_id_and_preserves_newer_selection() {
        let (_dir, mut library) = library();
        library.save("A", "a").unwrap();
        let confirmed = library.selected.clone().unwrap();
        library.new_draft();
        library.save("B", "b").unwrap();
        let newer = library.selected.clone();
        library.delete(&confirmed).unwrap();
        assert_eq!(library.selected, newer);
        assert_eq!(library.items.len(), 1);
        library.delete(&newer.unwrap()).unwrap();
        assert!(library.selected.is_none());
    }
    #[test]
    fn swift_library_is_untouched_and_rust_file_is_private() {
        let (dir, mut library) = library();
        let legacy = dir.path().join("Snippets.json");
        std::fs::write(&legacy, "original data").unwrap();
        library.save("A", "a").unwrap();
        assert_eq!(std::fs::read_to_string(legacy).unwrap(), "original data");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&library.path)
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
}

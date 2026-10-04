//! Shared, UI-independent workbench primitives. Filesystem and Git methods are
//! blocking and must be run on a background executor by a graphical host.
pub mod diff;
pub mod document;
pub mod editor;
pub mod git;
pub mod quick_open;
pub mod tree;

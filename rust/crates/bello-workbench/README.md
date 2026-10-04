# Bello shared workbench core

This crate is the canonical, UI-independent implementation shared by the Rust
BelloBox and BelloAgent applications. `bello-workbench-ui` supplies GPUI 0.2.2
views. This is a new Rust vertical slice, not a claim of Swift feature parity.

## Included

- Lazy, per-directory browsing; stable lossless `PathBuf` identities; directories
  before files; explicit truncation; no symlink-directory traversal.
- Read-only Git status, staged/worktree diffs, paginated history and full-hash
  commit-versus-first-parent diffs (root commits compare with the empty tree).
- UTF-8 text editing with incrementally maintained byte and UTF-16 line indexes.
- Change transactions, bounded inverse-edit undo/redo, CRLF-aware inserted
  newlines, literal search with wrap, Unicode-safe byte offsets.
- Explicit file-save action, changed-on-disk rejection, sibling staging file,
  permission preservation, file sync and atomic rename. The saved text is never
  silently truncated, transcoded, or normalized.

## Vim subset contract

Opt-in, off by default. Normal, Insert, character Visual and line Visual modes;
`h j k l`, arrows, `w b e`, `0 ^ $`, `gg G`, counts, `d c y` operators, doubled
line operators (`dd cc yy`), `x X D C`, `p P`, `i a I A o O`, `u`, Ctrl-R,
`/ ? n N`, and Escape. A whole insert/change is an undo transaction until a
cursor move, Escape, or the undo-memory chunk boundary. Counts combine for
operator motions. The unnamed register distinguishes characterwise/linewise
content. Clipboard access is supplied by the GUI, not this library.

This does not embed Vim, Neovim, or Zed's GPL editor. It does not implement Vim
macros, named registers, dot-repeat, Ex commands, regex search, text objects,
`f/t`, visual block mode, replace mode, or every Vim edge-case. Motions currently
count Unicode scalar values rather than grapheme clusters (combining sequences
can therefore be traversed in parts). No plugin API, LSP, syntax tree,
completion or syntax-highlighting engine is implied.

## Bounds and safety

All Git and filesystem methods are synchronous **background-worker APIs**.
The GUI dispatches them off its event thread and rejects superseded results.
Git reads accept cancellation, timeout, stdout and diagnostic budgets, drain
both pipes without deadlock, and kill/reap their process group on failure.
Arguments never pass through a shell. Pathspecs are literal; revisions must be
complete hashes. External diff, textconv, fsmonitor and pagers are disabled.
Git write operations are intentionally absent from this crate.

Editable files are at most 8 MiB. Larger text files show a read-only 256 KiB
preview; binary and invalid UTF-8 files are rejected. Undo history is bounded to
approximately 16 MiB and 2,000 committed groups; long active insert sessions are
chunked rather than retaining unbounded inverse edits. Tree reads have a 10,000
entry hard cap (GUI requests 2,000) and say when the inspected subset is partial.
Git stdout defaults to 16 MiB, stderr to 64 KiB; the GUI parses at most 20,000
diff display rows. Untracked files open as files rather than pretending there
is a tracked Git patch.

Saves reject path changes, external content/identity changes, symlinks and
multiply linked files. Atomic rename is not a portable compare-and-swap; an
uncooperative writer can still change a file between the final identity check
and rename. Only ordinary permissions are copied: platform-specific ACLs,
extended attributes and macOS file coordination remain follow-up work. Callers
must preserve the user's draft on any error. The UI guards file switches and
loading races; app hosts must guard window/app close using
`WorkbenchView::has_unsaved_changes`.

## Validation

`cargo test -p bello-workbench` runs real temporary-repository/process tests,
file conflict tests and editor transaction/index tests. The ignored
`process_fixture` is a subprocess fixture invoked by the active process tests.

`cargo run --release -p bello-workbench --example benchmark` prints JSON-line
microbenchmarks for 1 MiB indexing/typing/viewport lookup and a 20,000-row diff.
These CPU measurements do not measure GPU frame delivery, input-to-paint time,
real-repository watch performance, or macOS runtime behavior.

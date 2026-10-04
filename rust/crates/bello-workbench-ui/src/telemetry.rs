//! Optional content-free latency probes. Enable with BELLO_PERF_LOG=/path.
//! Metrics end at CPU paint or a following event-loop frame callback, never at
//! display scan-out/GPU presentation. Bounded nonblocking queue; one I/O thread.
use std::{
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    time::Instant,
};
const QUEUE_CAPACITY: usize = 1024;
static SINK: OnceLock<Option<Arc<Sink>>> = OnceLock::new();
static NEXT_EDITOR: AtomicU64 = AtomicU64::new(1);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InputKind {
    Key,
    Text,
    Composition,
    Scroll,
    Mixed,
}
impl InputKind {
    fn name(self) -> &'static str {
        match self {
            Self::Key => "key",
            Self::Text => "text",
            Self::Composition => "composition",
            Self::Scroll => "scroll",
            Self::Mixed => "mixed",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Paint,
    FollowingFrame,
}
impl Phase {
    fn name(self) -> &'static str {
        match self {
            Self::Paint => "editor_input_to_paint",
            Self::FollowingFrame => "editor_input_to_following_frame_callback",
        }
    }
}
#[derive(Clone, Copy, Debug)]
struct Metric {
    editor: u64,
    sequence: u64,
    phase: Phase,
    kind: InputKind,
    micros: u128,
    last_micros: u128,
    inputs: u32,
}
impl Metric {
    fn json(self, dropped: u64) -> String {
        format!(
            "{{\"component\":\"bello-workbench-ui\",\"event\":\"{}\",\"editor\":{},\"sequence\":{},\"input_kind\":\"{}\",\"duration_microseconds\":{},\"last_input_microseconds\":{},\"coalesced_inputs\":{},\"dropped_samples_total\":{},\"measurement\":\"CPU/event-loop callback; not GPU presentation\"}}\n",
            self.phase.name(),
            self.editor,
            self.sequence,
            self.kind.name(),
            self.micros,
            self.last_micros,
            self.inputs,
            dropped
        )
    }
}
#[derive(Default)]
struct Counters {
    attempted: AtomicU64,
    written: AtomicU64,
    dropped: AtomicU64,
    failed: AtomicBool,
}
struct Sink {
    sender: SyncSender<Metric>,
    counters: Arc<Counters>,
}
impl Sink {
    fn emit(&self, metric: Metric) {
        self.counters.attempted.fetch_add(1, Ordering::Relaxed);
        if self.counters.failed.load(Ordering::Relaxed) {
            self.counters.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        }
        match self.sender.try_send(metric) {
            Ok(()) => {}
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
                self.counters.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}
/// Process-wide logging counters. Written counts records, not inputs: inputs
/// coalesced by a frame yield a batch sample, and each batch has two phases.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EditorTelemetrySnapshot {
    pub enabled: bool,
    pub attempted_records: u64,
    pub written_records: u64,
    pub dropped_records: u64,
    pub writer_failed: bool,
}
pub fn editor_telemetry_snapshot() -> EditorTelemetrySnapshot {
    let Some(Some(sink)) = SINK.get() else {
        return EditorTelemetrySnapshot::default();
    };
    EditorTelemetrySnapshot {
        enabled: true,
        attempted_records: sink.counters.attempted.load(Ordering::Relaxed),
        written_records: sink.counters.written.load(Ordering::Relaxed),
        dropped_records: sink.counters.dropped.load(Ordering::Relaxed),
        writer_failed: sink.counters.failed.load(Ordering::Relaxed),
    }
}
fn writer_loop(path: PathBuf, receiver: Receiver<Metric>, counters: Arc<Counters>) {
    let result = (|| -> std::io::Result<()> {
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        while let Ok(first) = receiver.recv() {
            let mut batch = vec![first];
            batch.extend(receiver.try_iter().take(63));
            let dropped = counters.dropped.load(Ordering::Relaxed);
            let mut bytes = String::with_capacity(batch.len() * 360);
            for item in &batch {
                bytes.push_str(&item.json(dropped));
            }
            if let Err(error) = file.write_all(bytes.as_bytes()) {
                counters
                    .dropped
                    .fetch_add(batch.len() as u64, Ordering::Relaxed);
                return Err(error);
            }
            counters
                .written
                .fetch_add(batch.len() as u64, Ordering::Relaxed);
        }
        Ok(())
    })();
    if result.is_err() {
        counters.failed.store(true, Ordering::Relaxed);
        counters
            .dropped
            .fetch_add(receiver.try_iter().count() as u64, Ordering::Relaxed);
    }
}
fn configured_sink() -> Option<Arc<Sink>> {
    SINK.get_or_init(|| {
        let path = std::env::var_os("BELLO_PERF_LOG").filter(|p| !p.is_empty())?;
        let (sender, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let counters = Arc::new(Counters::default());
        let thread_counters = counters.clone();
        if std::thread::Builder::new()
            .name("bello-perf-writer".into())
            .spawn(move || writer_loop(path.into(), receiver, thread_counters))
            .is_err()
        {
            counters.failed.store(true, Ordering::Relaxed);
        }
        Some(Arc::new(Sink { sender, counters }))
    })
    .clone()
}
#[derive(Clone)]
pub(crate) struct Batch {
    sink: Arc<Sink>,
    editor: u64,
    sequence: u64,
    kind: InputKind,
    first: Instant,
    last: Instant,
    inputs: u32,
}
impl Batch {
    fn record(&self, phase: Phase) {
        let now = Instant::now();
        self.sink.emit(Metric {
            editor: self.editor,
            sequence: self.sequence,
            phase,
            kind: self.kind,
            micros: now.duration_since(self.first).as_micros(),
            last_micros: now.duration_since(self.last).as_micros(),
            inputs: self.inputs,
        });
    }
    pub fn paint(&self) {
        self.record(Phase::Paint);
    }
    pub fn following_frame(&self) {
        self.record(Phase::FollowingFrame);
    }
}
pub(crate) struct Tracker {
    sink: Arc<Sink>,
    editor: u64,
    sequence: u64,
    pending: Option<Batch>,
}
impl Tracker {
    pub fn configured() -> Option<Self> {
        let sink = configured_sink()?;
        Some(Self {
            sink,
            editor: NEXT_EDITOR.fetch_add(1, Ordering::Relaxed),
            sequence: 0,
            pending: None,
        })
    }
    pub fn input(&mut self, kind: InputKind, started: Instant) {
        if let Some(batch) = self.pending.as_mut() {
            batch.last = started;
            batch.inputs = batch.inputs.saturating_add(1);
            if batch.kind != kind {
                batch.kind = InputKind::Mixed;
            }
        } else {
            self.sequence = self.sequence.wrapping_add(1);
            self.pending = Some(Batch {
                sink: self.sink.clone(),
                editor: self.editor,
                sequence: self.sequence,
                kind,
                first: started,
                last: started,
                inputs: 1,
            });
        }
    }
    pub fn take_for_paint(&mut self, cursor_row: bool) -> Option<Batch> {
        if self
            .pending
            .as_ref()
            .is_some_and(|b| cursor_row || b.kind == InputKind::Scroll)
        {
            self.pending.take()
        } else {
            None
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn metric() -> Metric {
        Metric {
            editor: 1,
            sequence: 2,
            phase: Phase::Paint,
            kind: InputKind::Text,
            micros: 500,
            last_micros: 100,
            inputs: 3,
        }
    }
    #[test]
    fn queue_overflow_is_nonblocking_and_counted() {
        let (tx, rx) = mpsc::sync_channel(1);
        let counters = Arc::new(Counters::default());
        let sink = Sink {
            sender: tx,
            counters,
        };
        sink.emit(metric());
        sink.emit(metric());
        assert_eq!(sink.counters.attempted.load(Ordering::Relaxed), 2);
        assert_eq!(sink.counters.dropped.load(Ordering::Relaxed), 1);
        assert!(rx.try_recv().is_ok());
    }
    #[test]
    fn json_has_durations_batching_and_no_content_fields() {
        let line = metric().json(9);
        assert!(line.contains("\"duration_microseconds\":500"));
        assert!(line.contains("\"coalesced_inputs\":3"));
        assert!(line.contains("\"dropped_samples_total\":9"));
        assert!(
            !line.contains("file_path")
                && !line.contains("document_text")
                && !line.contains("key_character")
        );
        assert!(line.ends_with('\n'));
    }
    #[test]
    fn frame_coalescing_keeps_first_and_last_inputs() {
        let (tx, rx) = mpsc::sync_channel(8);
        let sink = Arc::new(Sink {
            sender: tx,
            counters: Arc::new(Counters::default()),
        });
        let mut t = Tracker {
            sink,
            editor: 1,
            sequence: 0,
            pending: None,
        };
        let start = Instant::now();
        t.input(InputKind::Text, start);
        t.input(InputKind::Text, Instant::now());
        assert!(t.take_for_paint(false).is_none());
        let b = t.take_for_paint(true).unwrap();
        assert_eq!(b.inputs, 2);
        b.paint();
        b.following_frame();
        assert_eq!(rx.try_iter().count(), 2);
        assert!(t.take_for_paint(true).is_none());
    }
    #[test]
    fn failed_writer_counts_dropped_records() {
        let (tx, _rx) = mpsc::sync_channel(1);
        let counters = Arc::new(Counters::default());
        counters.failed.store(true, Ordering::Relaxed);
        let sink = Sink {
            sender: tx,
            counters,
        };
        sink.emit(metric());
        assert_eq!(sink.counters.dropped.load(Ordering::Relaxed), 1);
    }
}

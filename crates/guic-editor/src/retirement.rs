//! Bounded, best-effort background release of retired editor history.
use super::EditorEdit;
use std::sync::{OnceLock, mpsc};

const QUEUED_BATCHES: usize = 8;
// Flat strings have cheap destruction; many per-line allocations dominate
// ordinary history disposal. Reserve byte-based deferral for very large lines.
const LARGE_BYTES: usize = 8 * 1024 * 1024;
const LARGE_LINES: usize = 256;

struct Reclaimer<T> {
    sender: mpsc::SyncSender<T>,
}
impl<T: Send + 'static> Reclaimer<T> {
    fn new(capacity: usize) -> Option<Self> {
        let (sender, receiver) = mpsc::sync_channel::<T>(capacity);
        std::thread::Builder::new()
            .name("guic-history-release".into())
            .spawn(move || {
                while let Ok(retired) = receiver.recv() {
                    drop(retired);
                }
            })
            .ok()?;
        Some(Self { sender })
    }

    fn retire(&self, value: T) {
        // Never wait for the worker or grow an unbounded retirement backlog.
        // Resource exhaustion falls back to ordinary synchronous destruction.
        if let Err(error) = self.sender.try_send(value) {
            drop(error);
        }
    }
}

pub(super) fn retire(edits: Vec<EditorEdit>) {
    if edits.is_empty() {
        return;
    }
    let mut bytes = 0usize;
    let mut lines = 0usize;
    let large = edits.iter().any(|edit| {
        bytes = bytes.saturating_add(edit.buffer.lines.byte_len());
        lines = lines.saturating_add(edit.buffer.line_count());
        bytes >= LARGE_BYTES || lines >= LARGE_LINES
    });
    if !large {
        return;
    }
    static RECLAIMER: OnceLock<Option<Reclaimer<Vec<EditorEdit>>>> = OnceLock::new();
    if let Some(reclaimer) = RECLAIMER.get_or_init(|| Reclaimer::new(QUEUED_BATCHES)) {
        reclaimer.retire(edits);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier, Mutex};

    struct Probe {
        dropped: mpsc::Sender<std::thread::ThreadId>,
        gate: Option<Arc<Barrier>>,
    }
    impl Drop for Probe {
        fn drop(&mut self) {
            if let Some(gate) = &self.gate {
                gate.wait();
                gate.wait();
            }
            self.dropped
                .send(std::thread::current().id())
                .expect("observer");
        }
    }

    #[test]
    fn retirement_releases_on_worker_and_bounds_backlog_without_waiting() {
        let worker = Reclaimer::new(1).expect("worker");
        let caller = std::thread::current().id();
        let gate = Arc::new(Barrier::new(2));
        let (sent, received) = mpsc::channel();
        worker.retire(Probe {
            dropped: sent.clone(),
            gate: Some(gate.clone()),
        });
        gate.wait();
        worker.retire(Probe {
            dropped: sent.clone(),
            gate: None,
        });
        worker.retire(Probe {
            dropped: sent,
            gate: None,
        });
        // The full queue releases its rejected value immediately, without waiting
        // for the blocked worker and without losing ownership of either batch.
        assert_eq!(received.recv().expect("fallback"), caller);
        gate.wait();
        assert_ne!(received.recv().expect("first batch"), caller);
        assert_ne!(received.recv().expect("queued batch"), caller);
    }

    #[test]
    fn closing_sender_drains_queued_values() {
        struct Count(Arc<Mutex<usize>>, mpsc::Sender<()>);
        impl Drop for Count {
            fn drop(&mut self) {
                *self.0.lock().expect("counter") += 1;
                self.1.send(()).expect("observer");
            }
        }
        let worker = Reclaimer::new(8).expect("worker");
        let count = Arc::new(Mutex::new(0));
        let (sent, received) = mpsc::channel();
        for _ in 0..8 {
            worker.retire(Count(count.clone(), sent.clone()));
        }
        drop(worker);
        for _ in 0..8 {
            received.recv().expect("released value");
        }
        assert_eq!(*count.lock().expect("counter"), 8);
    }
}

//! Lifecycle serializer: serializes operations that mutate `AppState` / config (create, connection
//! edit, clear key, delete, switch, one-click, stop, ensure_proxy). Health checks run **outside**
//! the lock; a generation token prevents stale proxy children from writing back after supersede.
//!
//! Lock layering (no self-deadlock):
//!   1. This serializer (`Mutex<()>`) — outermost, held for the whole command; never re-enter.
//!   2. `AppState` — short holds while reading/writing runtime fields.
//!   3. `config::update` — innermost, load-modify-save only.
//!
//! `ensure_proxy` / `start_proxy_for_profiles` never take this lock (callers hold it).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

pub struct Lifecycle {
    lock: Mutex<()>,
    generation: AtomicU64,
}

impl Default for Lifecycle {
    fn default() -> Self {
        Self::new()
    }
}

impl Lifecycle {
    pub fn new() -> Self {
        Lifecycle {
            lock: Mutex::new(()),
            generation: AtomicU64::new(1),
        }
    }

    /// Run `f` under the app-level mutex (atomizes read-config→spawn composites). Recovers from poison.
    pub fn with_serialized<T>(&self, f: impl FnOnce() -> T) -> T {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        f()
    }

    /// Called on clear-key / stop / switch: invalidates in-flight starts that finished probing outside the lock. Returns the new generation.
    pub fn bump_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn current_generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;
    use std::sync::Arc;

    #[test]
    fn with_serialized_runs_exclusively() {
        let lc = Arc::new(Lifecycle::new());
        let counter = Arc::new(AtomicU64::new(0));
        let max_seen = Arc::new(AtomicU64::new(0));
        let mut hs = vec![];
        for _ in 0..8 {
            let lc = lc.clone();
            let c = counter.clone();
            let m = max_seen.clone();
            hs.push(std::thread::spawn(move || {
                lc.with_serialized(|| {
                    let n = c.fetch_add(1, Ordering::SeqCst) + 1;
                    m.fetch_max(n, Ordering::SeqCst);
                    std::thread::sleep(std::time::Duration::from_millis(2));
                    c.fetch_sub(1, Ordering::SeqCst);
                });
            }));
        }
        for h in hs {
            h.join().unwrap();
        }
        assert_eq!(max_seen.load(Ordering::SeqCst), 1, "at most one thread inside serializer");
    }

    #[test]
    fn generation_bumps_monotonically() {
        let lc = Lifecycle::new();
        let g0 = lc.current_generation();
        let g1 = lc.bump_generation();
        assert!(g1 > g0);
        assert_eq!(lc.current_generation(), g1);
        let g2 = lc.bump_generation();
        assert!(g2 > g1);
    }
}

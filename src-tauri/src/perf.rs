//! Process-tree resource usage for the development performance overlay.
//!
//! WebView2 renders in child processes, so the app's real footprint is this
//! process plus every descendant.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

const REDISCOVER_EVERY: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    /// Private bytes of the Rust host process, in MiB.
    pub host_mb: f64,
    /// Private bytes of all WebView2 descendants, in MiB.
    pub webview_mb: f64,
    /// Sum of working sets across the tree, in MiB (overcounts shared pages).
    pub working_set_mb: f64,
    /// CPU usage of the whole tree, normalized to 0..100 across logical cores.
    pub cpu_percent: f64,
    pub process_count: usize,
}

struct Inner {
    system: System,
    tree: Vec<Pid>,
    discovered_at: Option<Instant>,
}

pub struct Monitor {
    root: Pid,
    cores: f64,
    inner: Mutex<Inner>,
}

impl Monitor {
    pub fn new() -> Self {
        Self {
            root: Pid::from_u32(std::process::id()),
            cores: std::thread::available_parallelism().map_or(1, |n| n.get()) as f64,
            inner: Mutex::new(Inner {
                system: System::new(),
                tree: Vec::new(),
                discovered_at: None,
            }),
        }
    }

    pub fn sample(&self) -> Stats {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let kind = ProcessRefreshKind::nothing().with_memory().with_cpu();

        let stale = inner
            .discovered_at
            .is_none_or(|t| t.elapsed() >= REDISCOVER_EVERY);
        if stale {
            inner.system.refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
            let parents = inner
                .system
                .processes()
                .iter()
                .filter_map(|(pid, p)| p.parent().map(|parent| (*pid, parent)));
            inner.tree = descendants_with_root(self.root, parents);
            inner.discovered_at = Some(Instant::now());
        } else {
            let Inner { system, tree, .. } = &mut *inner;
            system.refresh_processes_specifics(ProcessesToUpdate::Some(tree), true, kind);
        }

        let mut stats = Stats::default();
        let mib = |b: u64| b as f64 / (1024.0 * 1024.0);
        for pid in &inner.tree {
            let Some(p) = inner.system.process(*pid) else { continue };
            stats.process_count += 1;
            stats.working_set_mb += mib(p.memory());
            stats.cpu_percent += p.cpu_usage() as f64;
            if *pid == self.root {
                stats.host_mb = mib(p.virtual_memory());
            } else {
                stats.webview_mb += mib(p.virtual_memory());
            }
        }
        stats.cpu_percent /= self.cores;
        stats
    }
}

/// Returns `root` followed by every transitive child found in `parents`
/// (an iterator of `(child, parent)` pairs).
fn descendants_with_root(root: Pid, parents: impl Iterator<Item = (Pid, Pid)>) -> Vec<Pid> {
    let mut children: HashMap<Pid, Vec<Pid>> = HashMap::new();
    for (child, parent) in parents {
        children.entry(parent).or_default().push(child);
    }
    let mut seen = HashSet::from([root]);
    let mut out = vec![root];
    let mut i = 0;
    while i < out.len() {
        if let Some(kids) = children.get(&out[i]) {
            for kid in kids {
                if seen.insert(*kid) {
                    out.push(*kid);
                }
            }
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pid(n: u32) -> Pid {
        Pid::from_u32(n)
    }

    #[test]
    fn collects_transitive_children_only() {
        let pairs = [(2, 1), (3, 2), (4, 2), (5, 9), (6, 3)].map(|(c, p)| (pid(c), pid(p)));
        let mut tree = descendants_with_root(pid(1), pairs.into_iter());
        tree.sort();
        assert_eq!(tree, [1, 2, 3, 4, 6].map(pid));
    }

    #[test]
    fn survives_parent_cycles() {
        let pairs = [(2, 1), (1, 2)].map(|(c, p)| (pid(c), pid(p)));
        let tree = descendants_with_root(pid(1), pairs.into_iter());
        assert_eq!(tree, vec![pid(1), pid(2)]);
    }

    #[test]
    fn samples_own_process() {
        let stats = Monitor::new().sample();
        assert!(stats.process_count >= 1);
        assert!(stats.host_mb > 0.0);
    }
}

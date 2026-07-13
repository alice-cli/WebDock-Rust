//! Host CPU / RAM / disk for the web UI header.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use sysinfo::{Disks, System};

use crate::traits::MetricsProvider;
use crate::types::HostMetrics;

pub struct NativeMetrics {
    sys: Mutex<System>,
    last_cpu_refresh: Mutex<Option<Instant>>,
}

impl NativeMetrics {
    pub fn new() -> Self {
        let mut sys = System::new();
        // First CPU sample needs a prior refresh baseline.
        sys.refresh_cpu_all();
        sys.refresh_memory();
        Self {
            sys: Mutex::new(sys),
            last_cpu_refresh: Mutex::new(Some(Instant::now())),
        }
    }
}

impl Default for NativeMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsProvider for NativeMetrics {
    fn sample(&self) -> HostMetrics {
        let mut sys = self.sys.lock().unwrap_or_else(|e| e.into_inner());

        // sysinfo CPU % needs ≥~200ms between refresh_cpu_all calls.
        {
            let mut last = self
                .last_cpu_refresh
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let ready = last
                .map(|t| t.elapsed() >= Duration::from_millis(200))
                .unwrap_or(true);
            if ready {
                sys.refresh_cpu_all();
                *last = Some(Instant::now());
            }
        }
        sys.refresh_memory();

        let cpu = sys.global_cpu_usage() as f64;
        // sysinfo reports memory in bytes.
        let ram_total = sys.total_memory() as f64;
        let ram_used = sys.used_memory() as f64;
        let ram_pct = if ram_total > 0.0 {
            (ram_used / ram_total) * 100.0
        } else {
            0.0
        };

        let (disk_used, disk_total, disk_pct) = root_disk_usage();

        HostMetrics {
            cpu: round1(cpu),
            ram_pct: round1(ram_pct),
            disk_pct: round1(disk_pct),
            ram_used_gb: round1(ram_used / GB),
            ram_total_gb: round1(ram_total / GB),
            disk_used_gb: round1(disk_used / GB),
            disk_total_gb: round1(disk_total / GB),
        }
    }
}

const GB: f64 = 1024.0 * 1024.0 * 1024.0;

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// Prefer the root volume (macOS `/`, Linux `/`, Windows system drive).
fn root_disk_usage() -> (f64, f64, f64) {
    let disks = Disks::new_with_refreshed_list();
    let list = disks.list();
    if list.is_empty() {
        return (0.0, 0.0, 0.0);
    }

    // Prefer disk mounted at /
    let chosen = list
        .iter()
        .find(|d| d.mount_point() == std::path::Path::new("/"))
        .or_else(|| {
            list.iter().find(|d| {
                let m = d.mount_point().to_string_lossy();
                m == "/" || m == "C:\\" || m.starts_with("C:")
            })
        })
        .unwrap_or(&list[0]);

    let total = chosen.total_space() as f64;
    let avail = chosen.available_space() as f64;
    let used = (total - avail).max(0.0);
    let pct = if total > 0.0 {
        (used / total) * 100.0
    } else {
        0.0
    };
    (used, total, pct)
}

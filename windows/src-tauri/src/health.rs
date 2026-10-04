// Storage and device health: disk space, NVMe SMART (UDisks2, no root), memory
// and swap, CPU temperature. Sampled every 60 s; a missing source is skipped,
// never reported as a failure.

use std::collections::HashMap;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Ok,
    Warn,
    Bad,
}

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub id: String,
    pub label: String,
    pub value: String,
    pub level: Level,
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthReport {
    pub checks: Vec<Check>,
    pub worst: Level,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MemInfo {
    pub total_kb: u64,
    pub available_kb: u64,
    pub swap_total_kb: u64,
    pub swap_free_kb: u64,
}

pub struct Disk {
    pub mount: String,
    pub total: u64,
    pub free: u64,
}

pub struct Nvme {
    pub name: String,
    pub critical_warnings: Vec<String>,
    pub temp_c: Option<f64>,
    pub percent_used: Option<u8>,
}

pub struct Sample {
    pub disks: Vec<Disk>,
    pub mem: Option<MemInfo>,
    pub cpu_temp_c: Option<f64>,
    pub nvme: Vec<Nvme>,
}

pub fn parse_meminfo(text: &str) -> MemInfo {
    let mut m = MemInfo::default();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let (Some(key), Some(v)) = (it.next(), it.next().and_then(|v| v.parse::<u64>().ok())) else {
            continue;
        };
        match key {
            "MemTotal:" => m.total_kb = v,
            "MemAvailable:" => m.available_kb = v,
            "SwapTotal:" => m.swap_total_kb = v,
            "SwapFree:" => m.swap_free_kb = v,
            _ => {}
        }
    }
    m
}

fn pct(part: u64, whole: u64) -> f64 {
    if whole == 0 {
        0.0
    } else {
        part as f64 * 100.0 / whole as f64
    }
}

pub fn evaluate(s: &Sample) -> Vec<Check> {
    let mut out = Vec::new();
    for d in &s.disks {
        let free = pct(d.free, d.total);
        out.push(Check {
            id: format!("disk:{}", d.mount),
            label: format!("Disk {}", d.mount),
            value: format!("{:.0}% free · {:.0} GB", free, d.free as f64 / 1e9),
            level: if free < 10.0 {
                Level::Bad
            } else if free < 20.0 {
                Level::Warn
            } else {
                Level::Ok
            },
        });
    }
    if let Some(m) = &s.mem {
        let avail = pct(m.available_kb, m.total_kb);
        out.push(Check {
            id: "memory".into(),
            label: "Memory".into(),
            value: format!("{avail:.0}% available"),
            level: if avail < 10.0 { Level::Warn } else { Level::Ok },
        });
        if m.swap_total_kb > 0 {
            let used = pct(m.swap_total_kb - m.swap_free_kb, m.swap_total_kb);
            out.push(Check {
                id: "swap".into(),
                label: "Swap".into(),
                value: format!("{used:.0}% used"),
                level: if used > 50.0 { Level::Warn } else { Level::Ok },
            });
        }
    }
    if let Some(t) = s.cpu_temp_c {
        out.push(Check {
            id: "cpu".into(),
            label: "CPU temperature".into(),
            value: format!("{t:.0} °C"),
            level: if t > 90.0 {
                Level::Bad
            } else if t > 80.0 {
                Level::Warn
            } else {
                Level::Ok
            },
        });
    }
    for n in &s.nvme {
        let hot = n.temp_c.is_some_and(|t| t > 70.0);
        let worn = n.percent_used.is_some_and(|p| p > 90);
        let mut value = Vec::new();
        if let Some(p) = n.percent_used {
            value.push(format!("{p}% worn"));
        }
        if let Some(t) = n.temp_c {
            value.push(format!("{t:.0} °C"));
        }
        if !n.critical_warnings.is_empty() {
            value.push(format!("warning: {}", n.critical_warnings.join(", ")));
        }
        out.push(Check {
            id: format!("nvme:{}", n.name),
            label: format!("NVMe {}", n.name),
            value: value.join(" · "),
            level: if !n.critical_warnings.is_empty() || worn || hot { Level::Bad } else { Level::Ok },
        });
    }
    out
}

pub fn worst(checks: &[Check]) -> Level {
    checks.iter().map(|c| c.level).max().unwrap_or(Level::Ok)
}

fn disks() -> Vec<Disk> {
    let mut seen = Vec::new();
    let mut out = Vec::new();
    for mount in ["/", "/home"] {
        let path = std::ffi::CString::new(mount).unwrap();
        // SAFETY: statvfs only writes into the struct we own.
        let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statvfs(path.as_ptr(), &mut st) } != 0 || seen.contains(&st.f_fsid) {
            continue; // `/home` on the same filesystem as `/`: one row, not two
        }
        seen.push(st.f_fsid);
        let frsize = st.f_frsize as u64;
        out.push(Disk {
            mount: mount.into(),
            total: st.f_blocks as u64 * frsize,
            free: st.f_bavail as u64 * frsize,
        });
    }
    out
}

fn cpu_temp() -> Option<f64> {
    for dir in std::fs::read_dir("/sys/class/hwmon").ok()?.flatten() {
        let name = std::fs::read_to_string(dir.path().join("name")).unwrap_or_default();
        if name.trim() != "coretemp" {
            continue;
        }
        let max = std::fs::read_dir(dir.path())
            .ok()?
            .flatten()
            .filter(|f| f.file_name().to_string_lossy().ends_with("_input"))
            .filter_map(|f| std::fs::read_to_string(f.path()).ok()?.trim().parse::<f64>().ok())
            .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))));
        return max.map(|v| v / 1000.0);
    }
    None
}

fn strings(v: &Value) -> Vec<String> {
    match v {
        Value::Array(a) => a
            .iter()
            .filter_map(|x| match x {
                Value::Str(s) => Some(s.to_string()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

async fn nvme() -> Vec<Nvme> {
    let Ok(conn) = zbus::Connection::system().await else { return Vec::new() };
    let Ok(builder) = zbus::fdo::ObjectManagerProxy::builder(&conn)
        .destination("org.freedesktop.UDisks2")
        .and_then(|b| b.path("/org/freedesktop/UDisks2"))
    else {
        return Vec::new();
    };
    let Ok(manager) = builder.build().await else { return Vec::new() };
    let Ok(objects) = manager.get_managed_objects().await else { return Vec::new() };
    let mut out = Vec::new();
    for (path, interfaces) in objects {
        let Some((_, ctrl)) = interfaces
            .iter()
            .find(|(name, _)| name.as_str() == "org.freedesktop.UDisks2.NVMe.Controller")
        else {
            continue;
        };
        // /org/freedesktop/UDisks2/drives/KINGSTON_SNV3S500G_<serial> → KINGSTON
        let name = path.as_str().rsplit('/').next().unwrap_or("nvme").split('_').next().unwrap_or("nvme").to_string();
        let critical_warnings = ctrl.get("SmartCriticalWarning").map(|v| strings(v)).unwrap_or_default();
        let temp_c = match ctrl.get("SmartTemperature").map(|v| &**v) {
            Some(Value::U16(k)) if *k > 0 => Some(*k as f64 - 273.15),
            _ => None,
        };
        let percent_used = smart_percent_used(&conn, &path).await;
        out.push(Nvme { name, critical_warnings, temp_c, percent_used });
    }
    out
}

async fn smart_percent_used(conn: &zbus::Connection, path: &OwnedObjectPath) -> Option<u8> {
    let reply = conn
        .call_method(
            Some("org.freedesktop.UDisks2"),
            path.as_str(),
            Some("org.freedesktop.UDisks2.NVMe.Controller"),
            "SmartGetAttributes",
            &(HashMap::<&str, Value>::new(),),
        )
        .await
        .ok()?;
    let attrs: HashMap<String, OwnedValue> = reply.body().deserialize().ok()?;
    match attrs.get("percent_used").map(|v| &**v) {
        Some(Value::U8(p)) => Some(*p),
        _ => None,
    }
}

async fn sample() -> Sample {
    Sample {
        disks: disks(),
        mem: std::fs::read_to_string("/proc/meminfo").ok().map(|t| parse_meminfo(&t)),
        cpu_temp_c: cpu_temp(),
        nvme: nvme().await,
    }
}

/// Samples every 60 s (skipped while paused) and sends `health` to the island.
pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(60));
        loop {
            ticker.tick().await;
            if crate::integrations::PAUSED.load(std::sync::atomic::Ordering::Relaxed) {
                continue;
            }
            let checks = evaluate(&sample().await);
            let worst = worst(&checks);
            let _ = app.emit_to(crate::island::WINDOW_LABEL, "health", HealthReport { checks, worst });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const MEMINFO: &str = "MemTotal:       16000000 kB\nMemFree:  100 kB\nMemAvailable:    1000000 kB\nSwapTotal:       8000000 kB\nSwapFree:        2000000 kB\n";

    #[test]
    fn parses_meminfo() {
        let m = parse_meminfo(MEMINFO);
        assert_eq!(
            (m.total_kb, m.available_kb, m.swap_total_kb, m.swap_free_kb),
            (16_000_000, 1_000_000, 8_000_000, 2_000_000)
        );
    }

    fn sample() -> Sample {
        Sample {
            disks: vec![Disk { mount: "/".into(), total: 100, free: 50 }],
            mem: Some(parse_meminfo(MEMINFO)),
            cpu_temp_c: Some(50.0),
            nvme: vec![Nvme {
                name: "KINGSTON".into(),
                critical_warnings: vec![],
                temp_c: Some(37.0),
                percent_used: Some(4),
            }],
        }
    }

    fn level(checks: &[Check], id: &str) -> Level {
        checks.iter().find(|c| c.id == id).map(|c| c.level).unwrap()
    }

    #[test]
    fn thresholds() {
        let c = evaluate(&sample());
        assert_eq!(level(&c, "disk:/"), Level::Ok);
        assert_eq!(level(&c, "memory"), Level::Warn); // 6.25 % available
        assert_eq!(level(&c, "swap"), Level::Warn); // 75 % used
        assert_eq!(level(&c, "cpu"), Level::Ok);
        assert_eq!(level(&c, "nvme:KINGSTON"), Level::Ok);

        let mut s = sample();
        s.disks[0].free = 5;
        s.cpu_temp_c = Some(95.0);
        s.nvme[0].critical_warnings = vec!["spare".into()];
        let c = evaluate(&s);
        assert_eq!(level(&c, "disk:/"), Level::Bad);
        assert_eq!(level(&c, "cpu"), Level::Bad);
        assert_eq!(level(&c, "nvme:KINGSTON"), Level::Bad);
    }

    #[test]
    fn missing_sources_are_absent_not_errors() {
        let s = Sample { disks: vec![], mem: None, cpu_temp_c: None, nvme: vec![] };
        assert!(evaluate(&s).is_empty());
    }

    #[test]
    fn worst_level() {
        assert_eq!(worst(&evaluate(&sample())), Level::Warn);
        assert_eq!(worst(&[]), Level::Ok);
    }
}

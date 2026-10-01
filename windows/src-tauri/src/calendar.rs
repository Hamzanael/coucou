// The top-bar clock and its calendar.
//
// On Linux the island sits over GNOME's clock, so it takes over the clock's job:
// the time in the user's own format, and the events GNOME's clock menu shows.
// Those come from gnome-shell's calendar server (Evolution Data Server behind it,
// fed by Settings → Online Accounts), so no account or key lives in Coucou.

use serde::Serialize;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClockFormat {
    pub hour12: bool,
    pub show_date: bool,
    pub show_weekday: bool,
}

#[derive(Serialize, Clone)]
pub struct CalendarEvent {
    pub id: String,
    pub summary: String,
    /// Unix seconds.
    pub start: i64,
    pub end: i64,
}

#[cfg(target_os = "linux")]
pub use linux::{clock_format, events};

#[cfg(not(target_os = "linux"))]
pub fn clock_format() -> ClockFormat {
    ClockFormat { hour12: false, show_date: true, show_weekday: true }
}

#[cfg(not(target_os = "linux"))]
pub async fn events(_since: i64, _until: i64) -> Vec<CalendarEvent> {
    Vec::new()
}

#[cfg(target_os = "linux")]
mod linux {
    use std::collections::HashMap;
    use std::process::Command;
    use std::time::Duration;

    use futures_util::StreamExt;
    use zbus::zvariant::OwnedValue;

    use super::{CalendarEvent, ClockFormat};

    /// The first batch can take a moment while EDS opens each calendar.
    const FIRST_BATCH_WAIT: Duration = Duration::from_secs(4);
    /// Each calendar answers in its own signal; this much quiet means all are in.
    const SETTLE: Duration = Duration::from_millis(400);

    #[zbus::proxy(
        interface = "org.gnome.Shell.CalendarServer",
        default_service = "org.gnome.Shell.CalendarServer",
        default_path = "/org/gnome/Shell/CalendarServer"
    )]
    trait CalendarServer {
        fn set_time_range(&self, since: i64, until: i64, force_reload: bool) -> zbus::Result<()>;

        #[zbus(signal)]
        fn events_added_or_updated(
            &self,
            events: Vec<(String, String, i64, i64, HashMap<String, OwnedValue>)>,
        ) -> zbus::Result<()>;
    }

    fn gsetting(key: &str) -> Option<String> {
        let out = Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", key])
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().trim_matches('\'').to_string())
    }

    pub fn clock_format() -> ClockFormat {
        ClockFormat {
            hour12: gsetting("clock-format").as_deref() == Some("12h"),
            show_date: gsetting("clock-show-date").as_deref() != Some("false"),
            show_weekday: gsetting("clock-show-weekday").as_deref() == Some("true"),
        }
    }

    /// Events overlapping [since, until), sorted by start. Empty when there is no
    /// calendar server (another desktop) or no calendar configured.
    pub async fn events(since: i64, until: i64) -> Vec<CalendarEvent> {
        match fetch(since, until).await {
            Ok(events) => events,
            Err(err) => {
                crate::log::line(format!("calendar: {err}"));
                Vec::new()
            }
        }
    }

    async fn fetch(since: i64, until: i64) -> zbus::Result<Vec<CalendarEvent>> {
        let conn = zbus::Connection::session().await?;
        let server = CalendarServerProxy::new(&conn).await?;
        // Subscribe before asking, or the first calendar's answer can be missed.
        let mut updates = server.receive_events_added_or_updated().await?;
        server.set_time_range(since, until, true).await?;

        let mut by_id: HashMap<String, CalendarEvent> = HashMap::new();
        let mut wait = FIRST_BATCH_WAIT;
        while let Ok(Some(signal)) = tokio::time::timeout(wait, updates.next()).await {
            wait = SETTLE;
            let Ok(args) = signal.args() else { continue };
            for (id, summary, start, end, _) in args.events() {
                if *start < until && *end > since {
                    by_id.insert(
                        id.clone(),
                        CalendarEvent { id: id.clone(), summary: summary.clone(), start: *start, end: *end },
                    );
                }
            }
        }

        let mut events: Vec<CalendarEvent> = by_id.into_values().collect();
        events.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| a.summary.cmp(&b.summary)));
        Ok(events)
    }
}

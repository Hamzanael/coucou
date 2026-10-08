// Island window: placement on the chosen display, the two window sizes
// (full panel / invisible wake strip), click-through and the cursor poll.
//
// There is no notch on a PC, so the island is a black shape drawn at the top
// centre of the main display inside a borderless, transparent, always-on-top
// window that never takes focus.
//
// Windows and Linux share everything here except a handful of Win32 calls. On
// Linux the island runs under X11 (XWayland on a Wayland desktop, see main.rs):
// the cursor comes from GDK through Tauri, focus is refused through GTK, and
// there is no WebView2 drop target to fix up.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewWindow};

#[cfg(windows)]
use windows::core::BOOL;
#[cfg(windows)]
use windows::Win32::Foundation::{HWND, LPARAM, POINT};
#[cfg(windows)]
use windows::Win32::System::Ole::RevokeDragDrop;
#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, GetClassNameW, GetCursorPos, GetWindowLongPtrW, SetWindowLongPtrW,
    GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

/// Logical size of the full window — the largest island view, like the macOS panel.
pub const PANEL_W: f64 = 720.0;
pub const PANEL_H: f64 = 320.0;
/// Logical size of the invisible strip that wakes the island when it is hidden.
#[cfg(not(target_os = "linux"))]
pub const STRIP_W: f64 = 240.0;
#[cfg(not(target_os = "linux"))]
pub const STRIP_H: f64 = 6.0;
/// On Linux the hidden island is the top-bar clock, so the window shrinks to
/// the clock pill (NOTCH_W × NOTCH_H in layout.ts) instead of a strip.
#[cfg(target_os = "linux")]
pub const STRIP_W: f64 = 184.0;
#[cfg(target_os = "linux")]
pub const STRIP_H: f64 = 32.0;

pub const WINDOW_LABEL: &str = "island";

/// Margin around the island that still counts as "on the island", in logical px.
/// Wider than the macOS 6 pt because a click must never be swallowed.
const HIT_MARGIN: f64 = 14.0;

/// Whether the poll thread follows the global cursor to drive click-through and
/// hover. Not on Linux: under Wayland an X client only sees the pointer while it
/// is over the window, so a polled position goes stale the moment the pointer
/// leaves — the island would never see it come back, nor leave. And GNOME reads
/// an override-redirect window's input shape once, so click-through cannot be
/// toggled either. There the window is sized to the island itself (see
/// `apply_geometry`) and the page reads its own mouse events.
const POLL_TRACKS_CURSOR: bool = cfg!(not(target_os = "linux"));

#[derive(Serialize, Clone)]
pub struct CursorPayload {
    pub x: f64,
    pub y: f64,
}

#[derive(Serialize, Clone)]
pub struct ScreenInfo {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub scale: f64,
}

/// The island shape in window-logical coordinates, pushed by the front end.
/// The poll thread owns the click-through decision so it lands in the same 16 ms
/// tick as the cursor read — an IPC round trip here loses clicks.
#[derive(Clone, Copy, Default)]
pub struct IslandRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Wakes / parks the cursor poll thread so a hidden island costs literally nothing.
pub struct PollGate {
    active: Mutex<bool>,
    cv: Condvar,
    pub collapsed: AtomicBool,
    pub rect: Mutex<IslandRect>,
    /// Mirrors the window flag so we only call into Win32 when it changes.
    ignoring: AtomicBool,
    /// Held while the click-through flag is changed, so the poll thread's last
    /// tick can never re-apply it to a window that has just been collapsed.
    ignore_lock: Mutex<()>,
}

impl PollGate {
    pub fn new() -> Self {
        Self {
            active: Mutex::new(false),
            cv: Condvar::new(),
            collapsed: AtomicBool::new(true),
            rect: Mutex::new(IslandRect::default()),
            ignoring: AtomicBool::new(false),
            ignore_lock: Mutex::new(()),
        }
    }

    /// Collapses or expands the window's input handling: the collapsed window
    /// (wake strip or clock pill) always takes the mouse, and the next poll tick
    /// after expanding re-applies click-through from scratch.
    pub fn set_collapsed(&self, app: &AppHandle, collapsed: bool) {
        let _guard = self.ignore_lock.lock().unwrap();
        self.collapsed.store(collapsed, Ordering::Relaxed);
        #[cfg(not(target_os = "linux"))]
        set_ignore_cursor(app, false);
        #[cfg(target_os = "linux")]
        let _ = app;
        self.ignoring.store(false, Ordering::Relaxed);
    }

    pub fn set_rect(&self, rect: IslandRect) {
        *self.rect.lock().unwrap() = rect;
    }

    fn rect(&self) -> IslandRect {
        *self.rect.lock().unwrap()
    }

    pub fn set_active(&self, on: bool) {
        let mut guard = self.active.lock().unwrap();
        *guard = on;
        self.cv.notify_all();
    }

    fn wait_until_active(&self) {
        let mut guard = self.active.lock().unwrap();
        while !*guard {
            guard = self.cv.wait(guard).unwrap();
        }
    }

    fn is_active(&self) -> bool {
        *self.active.lock().unwrap()
    }
}

pub fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(WINDOW_LABEL)
}

#[cfg(windows)]
fn cursor_physical(_app: &AppHandle) -> Option<(f64, f64)> {
    let mut p = POINT::default();
    unsafe { GetCursorPos(&mut p).ok()? };
    Some((p.x as f64, p.y as f64))
}

/// GDK's pointer position, in physical desktop coordinates. Global under X11.
#[cfg(not(windows))]
fn cursor_physical(app: &AppHandle) -> Option<(f64, f64)> {
    let p = app.cursor_position().ok()?;
    Some((p.x, p.y))
}

/// Lets dropped files reach the app again.
///
/// wry installs its drop target by walking the webview's child windows **once**,
/// when the webview is created. WebView2 creates `Chrome_RenderWidgetHostHWND`
/// later and registers its own target on it; being the innermost window, that one
/// wins, and since the page has no HTML5 drop handler it refuses everything — the
/// "no drop" cursor, with nothing reaching Tauri. Revoking it makes OLE fall
/// through to the target wry registered on the parent widget, which is the one
/// that feeds Tauri's drag events.
///
/// Cheap and idempotent, so it is simply re-run whenever a drag might be starting.
/// WebKitGTK has no such second target, so there is nothing to do on Linux.
#[cfg(windows)]
pub fn unblock_webview_drops(app: &AppHandle) {
    for label in [WINDOW_LABEL, "settings"] {
        let Some(win) = app.get_webview_window(label) else { continue };
        let Some(hwnd) = hwnd_of(&win) else { continue };
        unsafe {
            let _ = EnumChildWindows(Some(hwnd), Some(revoke_render_widget), LPARAM(0));
        }
    }
}

#[cfg(not(windows))]
pub fn unblock_webview_drops(_app: &AppHandle) {}

#[cfg(windows)]
unsafe extern "system" fn revoke_render_widget(hwnd: HWND, _: LPARAM) -> BOOL {
    let mut name = [0u16; 64];
    let len = unsafe { GetClassNameW(hwnd, &mut name) };
    if len > 0 {
        let class = String::from_utf16_lossy(&name[..len as usize]);
        if class == "Chrome_RenderWidgetHostHWND" {
            let _ = unsafe { RevokeDragDrop(hwnd) };
        }
    }
    true.into()
}

/// True while the left mouse button is held — the only signal we get that a
/// drag might be in flight before it reaches the window.
#[cfg(windows)]
fn left_button_down() -> bool {
    unsafe { (GetAsyncKeyState(VK_LBUTTON.0 as i32) as u16 & 0x8000) != 0 }
}

/// On Linux a drag is picked up by the island shape itself: GTK's input shape
/// does not hide the window from XDND the way WS_EX_TRANSPARENT hides it from
/// OLE, so there is no need to widen the hit area while a button is held.
#[cfg(not(windows))]
fn left_button_down() -> bool {
    false
}

fn monitor_contains(m: &Monitor, x: f64, y: f64) -> bool {
    let p = m.position();
    let s = m.size();
    x >= p.x as f64
        && x < (p.x + s.width as i32) as f64
        && y >= p.y as f64
        && y < (p.y + s.height as i32) as f64
}

/// The display the island lives on: the primary one, or the one under the cursor.
fn target_monitor(app: &AppHandle, pref: &str) -> Option<Monitor> {
    let monitors = app.available_monitors().ok()?;
    if pref == "cursor" {
        if let Some((cx, cy)) = cursor_physical(app) {
            if let Some(m) = monitors.iter().find(|m| monitor_contains(m, cx, cy)) {
                return Some(m.clone());
            }
        }
    }
    app.primary_monitor()
        .ok()
        .flatten()
        .or_else(|| monitors.into_iter().next())
}

pub fn screen_info(app: &AppHandle, pref: &str) -> ScreenInfo {
    match target_monitor(app, pref) {
        Some(m) => {
            let scale = m.scale_factor();
            let p = m.position();
            let s = m.size();
            ScreenInfo {
                x: p.x as f64 / scale,
                y: p.y as f64 / scale,
                width: s.width as f64 / scale,
                height: s.height as f64 / scale,
                scale,
            }
        }
        None => ScreenInfo { x: 0.0, y: 0.0, width: 1920.0, height: 1080.0, scale: 1.0 },
    }
}

/// Logical window size: the wake strip or the full panel; on Linux, the island
/// itself as the front end last pushed it (the panel until it has).
fn window_size(app: &AppHandle, collapsed: bool) -> (f64, f64) {
    #[cfg(target_os = "linux")]
    {
        let rect = app.try_state::<crate::Shared>().map(|s| s.gate.rect()).unwrap_or_default();
        if rect.w > 0.0 && rect.h > 0.0 {
            return (rect.w.round(), rect.h.round());
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = app;
    if collapsed { (STRIP_W, STRIP_H) } else { (PANEL_W, PANEL_H) }
}

/// Places and sizes the window. `collapsed` picks the wake strip instead of the panel.
pub fn apply_geometry(app: &AppHandle, pref: &str, collapsed: bool) {
    let Some(win) = window(app) else { return };
    let Some(m) = target_monitor(app, pref) else { return };

    let scale = m.scale_factor();
    let mp = *m.position();
    let ms = *m.size();

    let (lw, lh) = window_size(app, collapsed);
    let pw = (lw * scale).round().max(1.0) as u32;
    let ph = (lh * scale).round().max(1.0) as u32;
    let x = mp.x + (ms.width as i32 - pw as i32) / 2;
    let y = mp.y;

    // GTK never shrinks a non-resizable window below its natural size, which left
    // the collapsed clock pill a 240×200 box eating clicks under the top bar. The
    // window is override-redirect, so no WM offers a resize handle either way.
    #[cfg(target_os = "linux")]
    let _ = win.set_resizable(true);
    let _ = win.set_size(PhysicalSize::new(pw, ph));
    let _ = win.set_position(PhysicalPosition::new(x, y));
    // Moving across displays can rescale the window: re-assert the physical size.
    let _ = win.set_size(PhysicalSize::new(pw, ph));
    let _ = win.set_always_on_top(true);
}

#[cfg(windows)]
fn hwnd_of(win: &WebviewWindow) -> Option<HWND> {
    let raw = win.hwnd().ok()?.0 as isize;
    if raw == 0 {
        return None;
    }
    Some(HWND(raw as *mut _))
}

/// WS_EX_NOACTIVATE keeps clicks from stealing focus; WS_EX_TOOLWINDOW keeps the
/// island out of Alt-Tab.
#[cfg(windows)]
pub fn make_non_activating(win: &WebviewWindow) {
    let Some(hwnd) = hwnd_of(win) else { return };
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let want = ex | WS_EX_NOACTIVATE.0 as isize | WS_EX_TOOLWINDOW.0 as isize;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, want);
    }
}

/// Temporarily allow activation so a text field inside the island can be typed in.
#[cfg(windows)]
pub fn set_activating(win: &WebviewWindow, activating: bool) {
    let Some(hwnd) = hwnd_of(win) else { return };
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let want = if activating {
            ex & !(WS_EX_NOACTIVATE.0 as isize)
        } else {
            ex | WS_EX_NOACTIVATE.0 as isize
        };
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, want);
    }
}

/// Takes the island away from the window manager by making it override-redirect,
/// the way menus and tooltips are. A managed window gets pushed below GNOME's top
/// bar, so the island would sit where the cursor poll does not think it is and
/// swallow no clicks; an override-redirect one stays exactly where it is placed,
/// is drawn above the top bar, and is never focused, raised or listed by the WM.
/// The flag only takes effect on map, hence the hide/show around it.
#[cfg(target_os = "linux")]
pub fn make_non_activating(win: &WebviewWindow) {
    use gtk::prelude::*;
    let Ok(gtk_win) = win.gtk_window() else { return };
    let was_visible = gtk_win.is_visible();
    gtk_win.hide();
    gtk_win.realize();
    if let Some(gdk_win) = gtk_win.window() {
        gdk_win.set_override_redirect(true);
    }
    if was_visible {
        gtk_win.show();
    }

    // WebKitGTK fires no DOM mouseout/mouseleave when the pointer leaves the
    // window and keeps faking mouse moves at the last position it saw, so the
    // page would think it is still hovered and never close. GTK knows better.
    gtk_win.add_events(gtk::gdk::EventMask::ENTER_NOTIFY_MASK | gtk::gdk::EventMask::LEAVE_NOTIFY_MASK);
    let page = win.clone();
    gtk_win.connect_enter_notify_event(move |_, event| {
        if event.detail() != gtk::gdk::NotifyType::Inferior {
            let _ = page.emit("pointer-entered", ());
        }
        gtk::glib::Propagation::Proceed
    });
    let page = win.clone();
    gtk_win.connect_leave_notify_event(move |_, event| {
        if event.detail() != gtk::gdk::NotifyType::Inferior {
            let _ = page.emit("pointer-left", ());
        }
        gtk::glib::Propagation::Proceed
    });

    // WebKit consumes button presses before they bubble to the window, so the
    // timestamp is taken on every widget, ahead of WebKit's own handler.
    record_press_times(gtk_win.upcast_ref());
}

/// Last X timestamp of a button press on the island: GNOME only lets a window
/// take focus for a user action it can date.
#[cfg(target_os = "linux")]
static LAST_PRESS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

#[cfg(target_os = "linux")]
fn record_press_times(widget: &gtk::Widget) {
    use gtk::prelude::*;
    widget.add_events(gtk::gdk::EventMask::BUTTON_PRESS_MASK);
    widget.connect_button_press_event(|_, event| {
        LAST_PRESS.store(event.time(), Ordering::Relaxed);
        gtk::glib::Propagation::Proceed
    });
    if let Some(container) = widget.downcast_ref::<gtk::Container>() {
        for child in container.children() {
            record_press_times(&child);
        }
    }
}

/// The keyboard shortcut is a user action too: its launch carries the event time
/// in the startup id (`…_TIME<ms>`), which lets the popup take focus.
pub fn startup_time(startup_id: &str) -> Option<u32> {
    let digits: String = startup_id.rsplit("_TIME").next()?.chars().take_while(|c| c.is_ascii_digit()).collect();
    if startup_id.contains("_TIME") { digits.parse().ok() } else { None }
}

#[cfg(target_os = "linux")]
pub fn note_user_time(time: u32) {
    LAST_PRESS.store(time, Ordering::Relaxed);
}

/// The island never takes the keyboard on Linux: it stays override-redirect for
/// good, since switching a window between managed and override-redirect left
/// GNOME routing no input to it. Typing happens in the compose window instead.
#[cfg(target_os = "linux")]
pub fn set_activating(_win: &WebviewWindow, _activating: bool) {}

/// Brings a normal (managed) window to the front with the keyboard, dated by
/// the island click that asked for it so GNOME lets it take focus.
#[cfg(target_os = "linux")]
pub fn present(win: &WebviewWindow) {
    use gtk::prelude::*;
    if let Ok(gtk_win) = win.gtk_window() {
        gtk_win.present_with_time(LAST_PRESS.load(Ordering::Relaxed));
    }
}

#[cfg(not(target_os = "linux"))]
pub fn present(win: &WebviewWindow) {
    let _ = win.set_focus();
}

/// Position, size and scale of the monitor the island lives on. Any change here
/// means the island has to be placed again.
fn current_screen_key(app: &AppHandle) -> Option<(i32, i32, u32, u32, u64)> {
    let pref = app
        .try_state::<crate::Shared>()
        .map(|s| s.settings.lock().unwrap().screen.clone())
        .unwrap_or_else(|| "primary".into());
    let m = target_monitor(app, &pref)?;
    let p = m.position();
    let size = m.size();
    Some((p.x, p.y, size.width, size.height, m.scale_factor().to_bits()))
}

/// Emits `cursor` (window-logical coordinates) at ~60 Hz while the island is
/// visible. Parked on a condvar the rest of the time.
pub fn spawn_cursor_poll(app: AppHandle, gate: Arc<PollGate>) {
    std::thread::spawn(move || {
        let mut was_down = false;
        // Remembered across wakes so a display change while hidden is noticed the
        // moment the island comes back.
        let mut last_screen: Option<(i32, i32, u32, u32, u64)> = None;
        loop {
            gate.wait_until_active();
            let mut last = (f64::MIN, f64::MIN);
            let mut ticks: u32 = 0;
            while gate.is_active() {
                std::thread::sleep(Duration::from_millis(16));

                // Monitors get plugged in, unplugged, rearranged and rescaled, and
                // an island pinned to coordinates that no longer exist is an island
                // nobody can reach. Checked about twice a second — the cursor poll
                // is already running, so this costs one monitor query.
                ticks = ticks.wrapping_add(1);
                if ticks % 30 == 0 {
                    let now = current_screen_key(&app);
                    if now.is_some() && now != last_screen {
                        let first = last_screen.is_none();
                        last_screen = now;
                        if !first {
                            crate::log::line("display layout changed — repositioning".to_string());
                            let _ = app.emit_to(WINDOW_LABEL, "screen-changed", ());
                        }
                    }
                }

                if !POLL_TRACKS_CURSOR {
                    continue;
                }
                let Some(win) = window(&app) else { continue };
                let Ok(origin) = win.outer_position() else { continue };
                let scale = win.scale_factor().unwrap_or(1.0);
                let Some((cx, cy)) = cursor_physical(&app) else { continue };
                let x = (cx - origin.x as f64) / scale;
                let y = (cy - origin.y as f64) / scale;
                let size = match win.inner_size() {
                    Ok(s) => (s.width as f64 / scale, s.height as f64 / scale),
                    Err(_) => (PANEL_W, PANEL_H),
                };
                if (x - last.0).abs() < 1.0 && (y - last.1).abs() < 1.0 {
                    continue;
                }
                last = (x, y);

                // Click-through: the window only takes the mouse over the island
                // shape. A small entry margin means the flag is already off by the
                // time a moving cursor reaches a button.
                let r = *gate.rect.lock().unwrap();
                let on_island = r.w > 0.0
                    && x >= r.x - HIT_MARGIN
                    && x <= r.x + r.w + HIT_MARGIN
                    && y >= r.y - HIT_MARGIN
                    && y <= r.y + r.h + HIT_MARGIN;

                // A file being dragged has to be able to find us. WS_EX_TRANSPARENT
                // — what click-through is on Windows — hides the window from
                // WindowFromPoint, so OLE finds no drop target and shows the "no
                // drop" cursor. macOS has no such problem: AppKit delivers drags to
                // registered destinations whatever ignoresMouseEvents says. So while
                // a button is held anywhere over the panel, the whole panel takes
                // the mouse, which also makes the drop zone as forgiving as the Mac's.
                // A press may be the start of a drag: make sure the drop target is
                // ours before the file arrives.
                let down = left_button_down();
                if down && !was_down {
                    let handle = app.clone();
                    let _ = app.run_on_main_thread(move || unblock_webview_drops(&handle));
                }
                was_down = down;

                let dragging = down
                    && x >= 0.0
                    && x <= size.0
                    && y >= 0.0
                    && y <= size.1;

                let accept = on_island || dragging;
                {
                    // The tick started before a collapse may only be finishing now:
                    // a click-through flag applied after it would leave the clock
                    // pill unable to take the mouse.
                    let _guard = gate.ignore_lock.lock().unwrap();
                    if !gate.collapsed.load(Ordering::Relaxed)
                        && gate.ignoring.load(Ordering::Relaxed) == accept
                    {
                        gate.ignoring.store(!accept, Ordering::Relaxed);
                        let _ = win.set_ignore_cursor_events(!accept);
                    }
                }

                let _ = win.emit("cursor", CursorPayload { x, y });
            }
        }
    });
}

#[cfg(not(target_os = "linux"))]
pub fn set_ignore_cursor(app: &AppHandle, ignore: bool) {
    if let Some(win) = window(app) {
        let _ = win.set_ignore_cursor_events(ignore);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_event_time_from_a_startup_id() {
        assert_eq!(startup_time("gnome-shell/Coucou/2755873-0-hamza_TIME1234567"), Some(1234567));
        assert_eq!(startup_time("_TIME42"), Some(42));
        assert_eq!(startup_time("no-time-here"), None);
        assert_eq!(startup_time(""), None);
    }
}

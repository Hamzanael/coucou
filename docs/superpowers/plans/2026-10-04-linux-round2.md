# Coucou Linux Round 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Linux island typeable and dismissable, list every Claude Code session, open projects in IntelliJ, clean stale git worktrees, and show storage/device health.

**Architecture:** The expanded island becomes a *managed* GNOME popup (focusable, under the top bar) while the clock pill and compact island stay an override-redirect window in the bar; Rust toggles the mode and forwards GTK focus/click timestamps. Sessions are a pure TypeScript store fed by hook events. Worktrees and health are new Rust modules with pure, unit-tested cores and thin I/O shells, each surfaced through Tauri commands/events and a new island view.

**Tech Stack:** Tauri 2 (Rust, gtk 0.18, zbus 5, libc), TypeScript + Vite, vitest (new, dev only), git CLI, UDisks2 over D-Bus.

**Spec:** `docs/superpowers/specs/2026-10-04-linux-round2-design.md`

## Global Constraints

- Work in `~/IdeaProjects/coucou`, branch `claude/topbar-clock-calendar`; the app lives in `windows/` (Rust in `windows/src-tauri/src`, TS in `windows/src`).
- **Never build, test or run npm/cargo on the laptop.** Every build and test runs in the fork's CI: `gh workflow run linux.yml -R Hamzanael/coucou --ref claude/topbar-clock-calendar`, then `gh run watch <id> -R Hamzanael/coucou` / `gh run view <id> --log-failed`. CI runs `cargo test --workspace` and (after Task 2) `npm test`.
- Linux-only behaviour goes behind `#[cfg(target_os = "linux")]` / the `domPointer`/`clock` boot flags; the Windows build must keep compiling (CI only builds Linux, so keep cfg arms symmetric by inspection).
- No new runtime dependencies beyond what is listed per task. No network calls except existing integrations (worktree checks use local refs only — never `git fetch`).
- Worktree cleanup: never `git worktree remove --force`, never delete branches, never remove anything that is not in the latest scan result, nothing pre-selected.
- Health sampling every 60 s; 0 % CPU when paused.
- Thresholds (verbatim from spec): disk < 10 % free; NVMe any critical warning, > 90 % used, > 70 °C; memory available < 10 %, swap > 50 % used; CPU > 90 °C. Sessions expire after 30 min without events. Worktree idle threshold 14 days (configurable).
- User-facing copy in English. Comments: minimal, explain *why* (repo style).
- Commits: `git -c user.name="Hamza" -c user.email=hnael99@gmail.com commit`, message ends with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- E2E keyboard tests may type **only after** `xprop -root _NET_ACTIVE_WINDOW` equals the island's X window id (never compare two possibly-empty values).

## Review Focus

1. **Focus-out during the mode switch itself** (unmap/remap emits focus-out) — the popup must not close the instant it opens. Pinned by Task 1 step "focus-lost before focus-gained is ignored" + e2e.
2. **Alert while the user is typing elsewhere** (approval request arrives) — the popup must appear without stealing keyboard focus. Pinned in Task 1 (`focus` false for `alert()`), e2e step checks `_NET_ACTIVE_WINDOW` unchanged.
3. **Hook payload without `session_id` / `cwd`** (older Claude Code, custom hooks) — must not crash or create a ghost row; folded into one "Session" row. Pinned by sessions test `missing ids`.
4. **Worktree paths with spaces, a worktree of a repo that has no `origin/HEAD`, or a dirty worktree selected for removal** — must report, not fail the whole batch. Pinned by `parse_porcelain` test with spaces, `default_branch` fallback test, and remove-result reporting.
5. **Health sources missing** (no UDisks2 NVMe, no coretemp, `/home` on the same fs as `/`) — those checks are simply absent/deduplicated, never an error state. Pinned by `evaluate` tests with `None` inputs and the fsid dedupe test.

---

## File Structure

| File | Responsibility |
|---|---|
| `windows/src-tauri/src/island.rs` (modify) | Window mode switch (bar ⇄ popup), GTK focus/click-time forwarding |
| `windows/src-tauri/src/lib.rs` (modify) | Register new commands, wire health poller |
| `windows/src-tauri/src/ide.rs` (create) | Find and launch IntelliJ |
| `windows/src-tauri/src/worktrees.rs` (create) | Parse/classify/scan/remove git worktrees |
| `windows/src-tauri/src/health.rs` (create) | Sample + evaluate disk, NVMe, memory, CPU temp |
| `windows/src-tauri/src/settings.rs` (modify) | `worktreeRoots`, `worktreeIdleDays` |
| `windows/src/core/sessions.ts` (create) | Pure session store |
| `windows/src/core/sessions.test.ts` (create) | vitest for the store |
| `windows/src/island/hooks.ts` (modify) | Feed hook events to the store; English labels |
| `windows/src/island/island.ts` (modify) | Window mode calls, close action, focus-lost |
| `windows/src/views/views.ts` (modify) | Header tabs + close button, sessions list in overview |
| `windows/src/views/sessions.ts` (create) | Session rows |
| `windows/src/views/worktrees.ts` (create) | Worktrees view |
| `windows/src/views/health.ts` (create) | Health view |
| `windows/src/core/{bridge,state,layout}.ts` (modify) | Types, commands, view names, System pill |
| `windows/dev/linux-e2e/` (create) | `vmouse.py`, `vkbd.py`, `shape.py`, `e2e.sh` (real-input harness) |
| `.github/workflows/linux.yml` (modify) | `npm test` step |

---

### Task 1: Expanded island = focusable popup; close button, Esc, click-elsewhere

**Files:**
- Modify: `windows/src-tauri/src/island.rs` (make_non_activating, set_activating; add set_popup)
- Modify: `windows/src-tauri/src/lib.rs` (`set_window_mode` command; `focus_window`)
- Modify: `windows/src/core/bridge.ts`, `windows/src/island/island.ts`, `windows/src/views/views.ts`, `windows/src/views/icons.ts`
- Create: `windows/dev/linux-e2e/{vmouse.py,vkbd.py,shape.py,e2e.sh}` (copy the scratchpad harness, see Step 6)

**Interfaces:**
- Produces (Rust): `island::set_popup(win: &WebviewWindow, popup: bool, focus: bool)`; command `set_window_mode(popup: bool, focus: bool)`; events `window-focus` (payload `bool`).
- Produces (TS): `Bridge.setWindowMode(popup: boolean, focus: boolean)`; `ViewActions.close(): void`.

- [ ] **Step 1: Rust — popup mode and focus forwarding.** In `island.rs`, below `make_non_activating` (Linux), add:

```rust
/// Last X timestamp of a button press on the island: GNOME only lets a window
/// take focus for a user action it can date.
#[cfg(target_os = "linux")]
static LAST_PRESS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Bar mode: override-redirect over the top bar, never focused. Popup mode: a
/// managed, undecorated, keep-above window GNOME places under the top bar and
/// can focus — the only way to type into it, see Esc, or learn of a click
/// elsewhere (focus-out). Switching needs an unmap/map.
#[cfg(target_os = "linux")]
pub fn set_popup(win: &WebviewWindow, popup: bool, focus: bool) {
    use gtk::gdk::WindowTypeHint;
    use gtk::prelude::*;
    let Ok(gtk_win) = win.gtk_window() else { return };
    let Some(gdk_win) = gtk_win.window() else { return };
    gtk_win.hide();
    gdk_win.set_override_redirect(!popup);
    gtk_win.set_type_hint(if popup { WindowTypeHint::Utility } else { WindowTypeHint::Normal });
    gtk_win.set_keep_above(true);
    gtk_win.set_skip_taskbar_hint(true);
    gtk_win.set_skip_pager_hint(true);
    gtk_win.set_accept_focus(popup);
    gtk_win.set_focus_on_map(false);
    gtk_win.show();
    if popup && focus {
        gtk_win.present_with_time(LAST_PRESS.load(std::sync::atomic::Ordering::Relaxed));
    }
}
```

In Linux `make_non_activating`, after the leave-notify block, add:

```rust
    gtk_win.add_events(gtk::gdk::EventMask::BUTTON_PRESS_MASK | gtk::gdk::EventMask::FOCUS_CHANGE_MASK);
    gtk_win.connect_button_press_event(|_, event| {
        LAST_PRESS.store(event.time(), std::sync::atomic::Ordering::Relaxed);
        gtk::glib::Propagation::Proceed
    });
    let page = win.clone();
    gtk_win.connect_focus_in_event(move |_, _| {
        let _ = page.emit("window-focus", true);
        gtk::glib::Propagation::Proceed
    });
    let page = win.clone();
    gtk_win.connect_focus_out_event(move |_, _| {
        let _ = page.emit("window-focus", false);
        gtk::glib::Propagation::Proceed
    });
```

Replace the Linux `set_activating` body (the keyboard grab) with:

```rust
#[cfg(target_os = "linux")]
pub fn set_activating(win: &WebviewWindow, activating: bool) {
    use gtk::prelude::*;
    if !activating {
        return;
    }
    if let Ok(gtk_win) = win.gtk_window() {
        gtk_win.present_with_time(LAST_PRESS.load(std::sync::atomic::Ordering::Relaxed));
    }
}
```

In `lib.rs`: in `focus_window`, guard the `win.set_focus()` call with `#[cfg(not(target_os = "linux"))]`. Add the command and register it in `generate_handler!`:

```rust
/// Linux: bar (pill / compact) ⇄ popup (expanded). No-op elsewhere.
#[tauri::command]
fn set_window_mode(app: AppHandle, popup: bool, focus: bool) {
    #[cfg(target_os = "linux")]
    if let Some(win) = island::window(&app) {
        island::set_popup(&win, popup, focus);
    }
    #[cfg(not(target_os = "linux"))]
    let _ = (app, popup, focus);
}
```

- [ ] **Step 2: TS — bridge + island wiring.** `bridge.ts` inside `Bridge`:

```ts
  /** Linux: expanded island = focusable popup under the top bar; else in the bar. */
  setWindowMode: (popup: boolean, focus: boolean) => call<void>("set_window_mode", { popup, focus }),
```

In `island.ts` add fields `private openedByClick = false;` and `private hadFocus = false;`. In `setMode(mode)`, right after `State.mode = mode;`:

```ts
    if (this.windowIsIsland && (mode === "expanded") !== (prev === "expanded")) {
      this.hadFocus = false;
      void Bridge.setWindowMode(mode === "expanded", mode === "expanded" && this.openedByClick);
      this.openedByClick = false;
    }
```

In the `mousedown` handler's `State.mode !== "expanded"` branch, set `this.openedByClick = true;` before `this.fsm.click()`. Add a `close()` method next to `collapse()`:

```ts
  /** × button, Esc, click elsewhere: straight back to the clock pill. */
  close() {
    State.isPinned = false;
    this.fsm.pinned = false;
    if (State.clock) this.fsm.forceHidden();
    else this.fsm.forcePetit();
  }
```

Change the Escape handler to call `this.close()`. In `makeWindowTheIsland()` add:

```ts
    void onEvent<boolean>("window-focus", (focused) => {
      // The unmap/map of a mode switch fires focus-out before the popup ever had
      // focus; only a focus that was gained and then lost means "clicked elsewhere".
      if (focused) this.hadFocus = true;
      else if (this.hadFocus && State.mode === "expanded" && !State.isPinned) this.close();
    });
```

Add `close: () => this.close(),` to the `actions` object in `build()`.

- [ ] **Step 3: Header close button.** `views.ts`: add `close(): void;` to `ViewActions`. In `buildHeader`, create `const closeBtn = h("button", { title: "Close", onclick: () => actions.close() }, svg(ICONS.xmark, 12, { stroke: 2.4 }));` and append it as the last child of `.header-actions`. (`ICONS.xmark` already exists — used by pill badges.)

- [ ] **Step 4: Commit and build.**

```bash
cd ~/IdeaProjects/coucou && git add -A windows && git -c user.name="Hamza" -c user.email=hnael99@gmail.com commit -m "Linux: expanded island is a focusable popup; close button, Esc, click-elsewhere

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>" && git push
gh workflow run linux.yml -R Hamzanael/coucou --ref claude/topbar-clock-calendar
```
Expected: CI green (`Test`, `Build and pack`).

- [ ] **Step 5: Install the artifact for testing.** `gh run download <id> -R Hamzanael/coucou -D <scratch>/ci`, `pkill -x coucou`, `dpkg-deb -x <deb> ~/.local/opt/coucou-dev`, start `~/.local/opt/coucou-dev/usr/bin/coucou` with `setsid nohup`.

- [ ] **Step 6: Commit the real-input harness** to `windows/dev/linux-e2e/` (vmouse.py, vkbd.py, shape.py, e2e.sh from the session scratchpad; `e2e.sh` uses `$(dirname "$0")`). Extend `e2e.sh` with, after "click shows the calendar":

```bash
  A=$(xprop -root _NET_ACTIVE_WINDOW | grep -o '0x[0-9a-f]*$')
  if [ -n "$A" ] && [ "$((A))" = "$((W))" ]; then ok "popup has keyboard focus"; else ko "popup not focused (active=$A island=$W)"; fi
  y=$(xwininfo -id "$W" | awk '/Absolute upper-left Y/{print $NF}')
  if [ "$y" -ge 30 ]; then ok "popup sits under the top bar (y=$y)"; else ko "popup y=$y"; fi
  if [ -n "$A" ] && [ "$((A))" = "$((W))" ]; then python3 "$S/vkbd.py" '\e'; sleep 1.2
    if near "$(geo)" 184x32; then ok "Esc closes to the pill"; else ko "Esc did nothing ($(geo))"; fi
  fi
```
Run `windows/dev/linux-e2e/e2e.sh 2` (flat accel on, restored after). Expected: all PASS. Then manually-scripted: open calendar by click, move pointer to (1280,700), click there (vmouse `click`) → expect `184x32` within 2 s ("click elsewhere closes").

- [ ] **Step 7: Commit harness + any fixes.**

---

### Task 2: Session store (pure TS) + vitest in CI

**Files:**
- Create: `windows/src/core/sessions.ts`, `windows/src/core/sessions.test.ts`
- Modify: `windows/package.json` (devDependency `vitest` `^3.2.0`, script `"test": "vitest run"`), `windows/package-lock.json` (regenerated in CI — see step 1), `.github/workflows/linux.yml`

**Interfaces:**
- Produces: `export interface HookPayload` (moved from hooks.ts), `export type SessionState = "thinking" | "working" | "approval" | "question" | "finished" | "error"`, `export interface Session { id: string; project: string; cwd: string; state: SessionState; step: string; updatedAt: number }`, `export class SessionStore { apply(p: HookPayload, now: number): Session | null; list(now: number): Session[]; busiest(now: number): SessionState | "idle"; }`, `export const SESSION_TTL_MS = 30 * 60_000`, `export function stepLabel(tool: string, input: Record<string, unknown>): string`.

- [ ] **Step 1: vitest + CI.** In `package.json` add `"test": "vitest run"` to scripts and `"vitest": "^3.2.0"` to devDependencies. The lock file can't be regenerated locally (no npm on the laptop): in `.github/workflows/linux.yml` change `run: npm ci` to `run: npm install --no-audit --no-fund` and add after it:

```yaml
      - name: Front-end tests
        run: npm test
```

- [ ] **Step 2: Write the failing tests** `windows/src/core/sessions.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { SessionStore, SESSION_TTL_MS, stepLabel } from "./sessions";

const ev = (name: string, extra: Record<string, unknown> = {}) => ({
  hook_event_name: name, session_id: "s1", cwd: "/home/u/IdeaProjects/analytickBE", ...extra,
});

describe("SessionStore", () => {
  it("creates one row per session id, named after the cwd", () => {
    const s = new SessionStore();
    s.apply(ev("SessionStart"), 1);
    s.apply(ev("SessionStart", { session_id: "s2", cwd: "/x/analytickFE" }), 2);
    expect(s.list(3).map((r) => r.project)).toEqual(["analytickFE", "analytickBE"]);
  });

  it("tracks state and the latest step", () => {
    const s = new SessionStore();
    s.apply(ev("UserPromptSubmit", { prompt: "fix the login bug" }), 1);
    expect(s.list(1)[0]).toMatchObject({ state: "thinking", step: "fix the login bug" });
    s.apply(ev("PreToolUse", { tool_name: "Bash", tool_input: { command: "git status" } }), 2);
    expect(s.list(2)[0]).toMatchObject({ state: "working", step: "Run · git status" });
    s.apply(ev("Stop"), 3);
    expect(s.list(3)[0].state).toBe("finished");
  });

  it("removes a session on SessionEnd and expires idle ones", () => {
    const s = new SessionStore();
    s.apply(ev("SessionStart"), 0);
    s.apply(ev("SessionStart", { session_id: "s2" }), 0);
    s.apply(ev("SessionEnd", { session_id: "s2" }), 1);
    expect(s.list(1)).toHaveLength(1);
    expect(s.list(SESSION_TTL_MS + 1)).toHaveLength(0);
  });

  it("folds payloads without ids into one 'Session' row", () => {
    const s = new SessionStore();
    s.apply({ hook_event_name: "PreToolUse", tool_name: "Read", tool_input: { file_path: "/a/b.ts" } }, 1);
    s.apply({ hook_event_name: "PreToolUse", tool_name: "Read", tool_input: { file_path: "/a/c.ts" } }, 2);
    expect(s.list(2)).toEqual([expect.objectContaining({ id: "", project: "Session", step: "Read · c.ts" })]);
  });

  it("busiest picks approval over error over working", () => {
    const s = new SessionStore();
    s.apply(ev("PreToolUse", { tool_name: "Bash", tool_input: {} }), 1);
    s.apply(ev("StopFailure", { session_id: "s2" }), 1);
    expect(s.busiest(1)).toBe("error");
    s.apply(ev("PermissionRequest", { session_id: "s3" }), 1);
    expect(s.busiest(1)).toBe("approval");
    expect(new SessionStore().busiest(1)).toBe("idle");
  });

  it("labels steps in English", () => {
    expect(stepLabel("Edit", { file_path: "/x/y/Main.kt" })).toBe("Edit · Main.kt");
    expect(stepLabel("Grep", { pattern: "foo" })).toBe("Search · foo");
    expect(stepLabel("Mystery", {})).toBe("Mystery");
  });
});
```

- [ ] **Step 3: Push and confirm the tests fail in CI** (module missing → `Front-end tests` fails).

- [ ] **Step 4: Implement** `windows/src/core/sessions.ts`:

```ts
// Claude Code sessions, one per `session_id`, built from hook events. Pure: the
// island feeds it events and reads rows; nothing here touches the DOM or Tauri.

export interface HookPayload {
  hook_event_name?: string;
  request_id?: string;
  session_id?: string;
  cwd?: string;
  message?: string;
  prompt?: string;
  tool_name?: string;
  tool_input?: Record<string, unknown>;
}

export type SessionState = "thinking" | "working" | "approval" | "question" | "finished" | "error";

export interface Session {
  id: string;
  project: string;
  cwd: string;
  state: SessionState;
  step: string;
  updatedAt: number;
}

export const SESSION_TTL_MS = 30 * 60_000;

const TOOL_LABELS: Record<string, string> = {
  Bash: "Run", PowerShell: "Run", Read: "Read", Write: "Write", Edit: "Edit", MultiEdit: "Edit",
  Glob: "Find", Grep: "Search", WebSearch: "Web search", WebFetch: "Fetch", TodoWrite: "Tasks",
  Task: "Agent", Agent: "Agent", LS: "List", NotebookEdit: "Notebook",
};

function lastPathComponent(p: string): string {
  const cleaned = p.replace(/[\\/]+$/, "");
  return cleaned.slice(Math.max(cleaned.lastIndexOf("/"), cleaned.lastIndexOf("\\")) + 1);
}

export function stepLabel(tool: string, input: Record<string, unknown>): string {
  const label = TOOL_LABELS[tool] ?? tool;
  const str = (k: string) => (typeof input[k] === "string" && input[k] ? (input[k] as string) : null);
  const detail =
    str("command")?.slice(0, 40) ??
    (str("file_path") ?? str("path") ? lastPathComponent((str("file_path") ?? str("path"))!) : null) ??
    str("query")?.slice(0, 40) ??
    str("pattern")?.slice(0, 40);
  return detail ? `${label} · ${detail}` : label;
}

const PRIORITY: SessionState[] = ["approval", "error", "question", "working", "thinking", "finished"];

export class SessionStore {
  private sessions = new Map<string, Session>();

  apply(p: HookPayload, now: number): Session | null {
    const id = p.session_id ?? "";
    const name = p.hook_event_name ?? "";
    if (name === "SessionEnd") {
      this.sessions.delete(id);
      return null;
    }
    const cwd = p.cwd ?? this.sessions.get(id)?.cwd ?? "";
    const s: Session = this.sessions.get(id) ?? {
      id, cwd, project: lastPathComponent(cwd) || "Session", state: "thinking", step: "", updatedAt: now,
    };
    s.updatedAt = now;
    if (cwd && !s.cwd) {
      s.cwd = cwd;
      s.project = lastPathComponent(cwd);
    }
    switch (name) {
      case "UserPromptSubmit":
        s.state = "thinking";
        s.step = (p.prompt ?? p.message ?? "").slice(0, 60);
        break;
      case "PreToolUse":
        s.state = "working";
        s.step = stepLabel(p.tool_name ?? "Tool", p.tool_input ?? {});
        break;
      case "PostToolUse":
        if (s.state !== "approval") s.state = "working";
        break;
      case "PostToolUseFailure":
        s.step = `${s.step} — failed`;
        break;
      case "PermissionRequest":
        s.state = "approval";
        s.step = stepLabel(p.tool_name ?? "Tool", p.tool_input ?? {});
        break;
      case "Notification":
        if ((p.message ?? "").endsWith("?")) {
          s.state = "question";
          s.step = p.message ?? "";
        }
        break;
      case "Stop":
        s.state = "finished";
        break;
      case "StopFailure":
        s.state = "error";
        break;
    }
    this.sessions.set(id, s);
    return s;
  }

  list(now: number): Session[] {
    for (const [id, s] of this.sessions) {
      if (now - s.updatedAt > SESSION_TTL_MS) this.sessions.delete(id);
    }
    return [...this.sessions.values()].sort((a, b) => b.updatedAt - a.updatedAt);
  }

  busiest(now: number): SessionState | "idle" {
    const states = new Set(this.list(now).map((s) => s.state));
    return PRIORITY.find((p) => states.has(p)) ?? "idle";
  }
}
```

- [ ] **Step 5: Push; CI `Front-end tests` passes (6 tests).**

- [ ] **Step 6: Commit** (`Sessions: one store entry per Claude Code session (pure, tested)`).

---

### Task 3: Sessions in the island + open in IntelliJ

**Files:**
- Create: `windows/src-tauri/src/ide.rs`, `windows/src/views/sessions.ts`
- Modify: `windows/src-tauri/src/lib.rs` (replace `open_in_vscode` with `open_in_ide`), `windows/src/core/bridge.ts`, `windows/src/core/state.ts`, `windows/src/island/hooks.ts`, `windows/src/island/island.ts`, `windows/src/views/views.ts`, `windows/src/views/integrations.ts`, `windows/src/style.css`

**Interfaces:**
- Consumes: `SessionStore`, `Session`, `HookPayload`, `stepLabel` (Task 2).
- Produces: Rust `ide::resolve(candidates: &[PathBuf], on_path: Option<PathBuf>) -> Option<PathBuf>`, command `open_in_ide(path: Option<String>) -> bool`; TS `Bridge.openInIde(path: string | null): Promise<boolean | null>`, `State.sessions: SessionStore`.

- [ ] **Step 1: Failing Rust test** in new `ide.rs`:

```rust
// Opens a project in IntelliJ: the JetBrains Toolbox launcher script first,
// then an `idea` on PATH.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The first candidate that exists, else whatever PATH offers.
pub fn resolve(candidates: &[PathBuf], on_path: Option<PathBuf>) -> Option<PathBuf> {
    candidates.iter().find(|p| p.is_file()).cloned().or(on_path)
}

fn candidates() -> Vec<PathBuf> {
    let home = crate::platform::home();
    vec![
        home.join(".local/share/JetBrains/Toolbox/scripts/idea"),
        PathBuf::from("/snap/bin/intellij-idea-ultimate"),
        PathBuf::from("/snap/bin/intellij-idea-community"),
    ]
}

/// Opens `path` in IntelliJ. False when no launcher was found or it failed to start.
pub fn open(path: Option<&str>) -> bool {
    let Some(launcher) = resolve(&candidates(), crate::find_on_path("idea")) else { return false };
    let mut cmd = Command::new(launcher);
    if let Some(p) = path.filter(|p| !p.is_empty() && Path::new(p).is_dir()) {
        cmd.arg(p);
    }
    crate::platform::quiet(&mut cmd).spawn().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_an_existing_candidate_over_path() {
        let dir = std::env::temp_dir().join(format!("coucou-ide-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("idea");
        std::fs::write(&script, "#!/bin/sh\n").unwrap();
        let got = resolve(&[dir.join("missing"), script.clone()], Some(PathBuf::from("/usr/bin/idea")));
        assert_eq!(got, Some(script));
    }

    #[test]
    fn falls_back_to_path_then_none() {
        assert_eq!(resolve(&[PathBuf::from("/nope/idea")], Some(PathBuf::from("/p/idea"))), Some(PathBuf::from("/p/idea")));
        assert_eq!(resolve(&[PathBuf::from("/nope/idea")], None), None);
    }
}
```

Make `find_on_path` in `lib.rs` `pub(crate)` (both cfg variants). Replace the `open_in_vscode` command with:

```rust
#[tauri::command]
fn open_in_ide(path: Option<String>) -> bool {
    if ide::open(path.as_deref()) {
        return true;
    }
    if let Some(p) = path.as_deref().filter(|p| !p.is_empty()) {
        platform::open_folder(p);
    }
    false
}
```
Add `mod ide;`, swap the name in `generate_handler!`.

- [ ] **Step 2: TS bridge + state.** In `bridge.ts` replace `openInVSCode` with `openInIde: (path: string | null) => call<boolean>("open_in_ide", { path }),`. In `state.ts`: `import { SessionStore } from "./sessions";`, add field `sessions = new SessionStore();`, rename the claude agent's display name `"VS Code"` → `"Claude"`. Replace every `openInVSCode` call (`island.ts` openTerminal/openTarget, `views/integrations.ts`) with `openInIde`, and the label `"Open Visual Studio Code"` → `"Open in IntelliJ"`. In `views.ts` `buildPill`, drop the `"VS Code"` special case (`const label = task.name;`).

- [ ] **Step 3: hooks.ts.** Delete the local `HookPayload`, `TOOL_LABELS`, `stepLabel`, `aliasProjectName`/`PROJECT_ALIASES`; import `{ type HookPayload, stepLabel } from "../core/sessions"`. At the top of `handleHook` (after the paused check) add `State.sessions.apply(payload, Date.now());`. Keep all existing cases (they drive the aggregate Claude pill, approvals and sounds), but replace the `PreToolUse` step with `stepLabel(...)` from the import, and after the `switch` (before `State.notify()`) add:

```ts
  const busiest = State.sessions.busiest(Date.now());
  const claude = State.tasks.find((t) => t.id === CLAUDE_ID);
  if (claude && busiest !== "idle" && claude.state !== "approval") claude.state = busiest;
```
In `clearSession()` set `t.name = "Claude"`.

- [ ] **Step 4: Session rows view** `windows/src/views/sessions.ts`:

```ts
// The overview's session list: one row per live Claude Code session.

import { Bridge } from "../core/bridge";
import { State } from "../core/state";
import type { Session } from "../core/sessions";
import { timeAgo } from "./integrations";
import { h, dot } from "./dom";

const STATE_COLOR: Record<Session["state"], string> = {
  thinking: "#A78BFA", working: "#3B9EFF", approval: "#F5A524",
  question: "#22D3EE", finished: "#34D399", error: "#F4505E",
};

export function renderSessions(sessions: Session[]): HTMLElement {
  const list = h("div", { class: "sessions" });
  for (const s of sessions.slice(0, 4)) {
    list.append(
      h("button", {
        class: "session-row",
        title: s.cwd ? `Open ${s.cwd} in IntelliJ` : s.project,
        onclick: () => void Bridge.openInIde(s.cwd || null),
      },
        dot(STATE_COLOR[s.state], 7),
        h("b", { text: s.project }),
        h("span", { class: "session-step", text: s.step || s.state }),
        h("span", { class: "session-age", text: timeAgo(s.updatedAt) }),
      ),
    );
  }
  if (sessions.length > 4) list.append(h("div", { class: "session-more", text: `+${sessions.length - 4} more` }));
  return list;
}

export const liveSessions = () => State.sessions.list(Date.now());
```

In `views.ts` `buildOverview.sync()`, before the `if (task && sessionActive)` branch insert:

```ts
      const sessions = liveSessions();
      if (task?.id === "integration_claude" && sessions.length > 0) {
        const key = sessions.map((s) => `${s.id}${s.state}${s.step}${Math.floor(s.updatedAt / 60_000)}`).join("|");
        if (mode !== "card" || cardKey !== key) {
          clear(leftBody);
          leftBody.append(renderSessions(sessions));
          mode = "card";
          cardKey = key;
        }
      } else
```
(so the existing `if (task && sessionActive) … else if (task) …` chain becomes the `else` arm). Import `renderSessions, liveSessions` from `./sessions`.

CSS (`style.css`, append):

```css
.sessions { display: flex; flex-direction: column; gap: 4px; padding: 10px 12px 10px 96px; height: 100%; overflow: hidden; }
.session-row { display: flex; align-items: center; gap: 7px; border: 0; background: rgba(255,255,255,.04); color: var(--ink); border-radius: 9px; padding: 5px 9px; font: inherit; font-size: 11.5px; text-align: left; cursor: default; }
.session-row:hover { background: rgba(255,255,255,.08); }
.session-step { flex: 1; min-width: 0; color: var(--dim); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.session-age { color: var(--dim-3); font-size: 10.5px; }
.session-more { color: var(--dim-3); font-size: 10.5px; padding-left: 9px; }
```

- [ ] **Step 5: Push; CI green** (Rust `ide` tests + front-end tests + build).

- [ ] **Step 6: E2E.** Install the artifact; with this Claude Code session running, open the island (vmouse hover + click on the Home tab at window x≈69,y≈24) and `xwd` the window: expect at least one row titled `IdeaProjects`/project name with a `Run · …` step. Click the row → `pgrep -f JetBrains` shows IntelliJ started/focused (or a new window). Commit any fixes.

---

### Task 4: Stale worktrees

**Files:**
- Create: `windows/src-tauri/src/worktrees.rs`, `windows/src/views/worktrees.ts`
- Modify: `windows/src-tauri/src/settings.rs`, `windows/src-tauri/src/lib.rs`, `windows/src/core/{bridge,layout,state}.ts`, `windows/src/views/{views,icons}.ts`, `windows/src/style.css`, `windows/src/settings/main.ts`

**Interfaces:**
- Produces (Rust): `worktrees::parse_porcelain(&str) -> Vec<Entry>`, `worktrees::classify(&Facts, idle_days: u64) -> Option<Reason>`, `worktrees::scan(roots: &[PathBuf], idle_days: u64) -> Vec<Stale>`, `worktrees::remove(stale: &[Stale], paths: &[String]) -> Vec<Removal>`; commands `worktrees_scan() -> Vec<Stale>`, `worktrees_remove(paths: Vec<String>) -> Vec<Removal>`. Settings fields `worktree_roots: Vec<String>` (default `["~/IdeaProjects"]`), `worktree_idle_days: u64` (default 14), both `#[serde(default = …)]`.
- Produces (TS): `Bridge.worktreesScan()`, `Bridge.worktreesRemove(paths)`, types `StaleWorktree { repo: string; path: string; branch: string | null; reason: "missing" | "merged" | "upstreamGone" | "idle"; idleDays: number | null; sizeKb: number | null }`, `WorktreeRemoval { path: string; ok: boolean; message: string }`; view name `"worktrees"`.

- [ ] **Step 1: Failing tests** at the bottom of `worktrees.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const PORCELAIN: &str = "worktree /home/u/IdeaProjects/app\nHEAD 1111\nbranch refs/heads/main\n\n\
worktree /home/u/IdeaProjects/app/.claude/worktrees/fix login\nHEAD 2222\nbranch refs/heads/claude/fix-login\n\n\
worktree /tmp/gone-wt\nHEAD 3333\ndetached\nprunable gitdir file points to non-existent location\n";

    #[test]
    fn parses_entries_including_spaces_and_prunable() {
        let e = parse_porcelain(PORCELAIN);
        assert_eq!(e.len(), 3);
        assert_eq!(e[1].path, "/home/u/IdeaProjects/app/.claude/worktrees/fix login");
        assert_eq!(e[1].branch.as_deref(), Some("claude/fix-login"));
        assert!(e[2].prunable);
        assert_eq!(e[2].branch, None);
    }

    fn facts() -> Facts {
        Facts { exists: true, merged: false, upstream_gone: false, clean: true, idle_days: Some(1) }
    }

    #[test]
    fn classifies_by_priority() {
        assert_eq!(classify(&Facts { exists: false, ..facts() }, 14), Some(Reason::Missing));
        assert_eq!(classify(&Facts { merged: true, clean: false, ..facts() }, 14), Some(Reason::Merged));
        assert_eq!(classify(&Facts { upstream_gone: true, ..facts() }, 14), Some(Reason::UpstreamGone));
        assert_eq!(classify(&Facts { idle_days: Some(20), ..facts() }, 14), Some(Reason::Idle));
        assert_eq!(classify(&Facts { idle_days: Some(20), clean: false, ..facts() }, 14), None);
        assert_eq!(classify(&facts(), 14), None);
    }

    #[test]
    fn default_branch_falls_back() {
        assert_eq!(pick_default_branch(Some("origin/develop"), &["main".into()]), "origin/develop");
        assert_eq!(pick_default_branch(None, &["master".into(), "x".into()]), "master");
        assert_eq!(pick_default_branch(None, &["main".into(), "master".into()]), "main");
        assert_eq!(pick_default_branch(None, &[]), "HEAD");
    }

    #[test]
    fn remove_refuses_paths_not_in_the_scan() {
        let r = remove(&[], &["/etc".to_string()]);
        assert_eq!(r.len(), 1);
        assert!(!r[0].ok);
    }
}
```

- [ ] **Step 2: Push; CI `Test` fails** (module missing).

- [ ] **Step 3: Implement** `worktrees.rs` (above the tests):

```rust
// Stale git worktrees under the configured roots: missing, merged / upstream
// gone, or clean and idle. Local refs only — no fetch, no network. Removal is
// `git worktree remove` without --force, then prune; branches are never touched.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

use serde::Serialize;

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub path: String,
    pub branch: Option<String>,
    pub prunable: bool,
}

pub fn parse_porcelain(text: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    for block in text.split("\n\n").filter(|b| !b.trim().is_empty()) {
        let mut entry = Entry { path: String::new(), branch: None, prunable: false };
        for line in block.lines() {
            if let Some(p) = line.strip_prefix("worktree ") {
                entry.path = p.to_string();
            } else if let Some(b) = line.strip_prefix("branch refs/heads/") {
                entry.branch = Some(b.to_string());
            } else if line.starts_with("prunable") {
                entry.prunable = true;
            }
        }
        if !entry.path.is_empty() {
            out.push(entry);
        }
    }
    out
}

pub struct Facts {
    pub exists: bool,
    pub merged: bool,
    pub upstream_gone: bool,
    pub clean: bool,
    pub idle_days: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Reason {
    Missing,
    Merged,
    UpstreamGone,
    Idle,
}

pub fn classify(f: &Facts, idle_days: u64) -> Option<Reason> {
    if !f.exists {
        Some(Reason::Missing)
    } else if f.merged {
        Some(Reason::Merged)
    } else if f.upstream_gone {
        Some(Reason::UpstreamGone)
    } else if f.clean && f.idle_days.is_some_and(|d| d >= idle_days) {
        Some(Reason::Idle)
    } else {
        None
    }
}

pub fn pick_default_branch(origin_head: Option<&str>, local: &[String]) -> String {
    if let Some(h) = origin_head {
        return h.to_string();
    }
    for name in ["main", "master"] {
        if local.iter().any(|b| b == name) {
            return name.to_string();
        }
    }
    "HEAD".to_string()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stale {
    pub repo: String,
    pub path: String,
    pub branch: Option<String>,
    pub reason: Reason,
    pub idle_days: Option<u64>,
    pub size_kb: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Removal {
    pub path: String,
    pub ok: bool,
    pub message: String,
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).to_string())
}

fn lines(s: Option<String>) -> Vec<String> {
    s.map(|s| s.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()).unwrap_or_default()
}

/// Repos directly in each root (a `.git` directory — worktrees have a `.git` file).
fn repos(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for root in roots {
        if root.join(".git").is_dir() {
            out.push(root.clone());
        }
        let Ok(dir) = std::fs::read_dir(root) else { continue };
        for e in dir.flatten() {
            if e.path().join(".git").is_dir() {
                out.push(e.path());
            }
        }
    }
    out
}

fn days_since(t: SystemTime) -> Option<u64> {
    SystemTime::now().duration_since(t).ok().map(|d| d.as_secs() / 86_400)
}

/// Last activity: the newer of the HEAD commit and the worktree's own git dir
/// (checkout / commit / index refresh), so a fresh worktree of an old commit is
/// not "idle" on day one.
fn idle_days(path: &Path) -> Option<u64> {
    let commit = git(path, &["log", "-1", "--format=%ct"])
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(|secs| SystemTime::UNIX_EPOCH + Duration::from_secs(secs));
    let gitdir = git(path, &["rev-parse", "--absolute-git-dir"]).map(|s| PathBuf::from(s.trim()));
    let touched = gitdir.iter().flat_map(|d| ["HEAD", "index"].map(|f| d.join(f)))
        .filter_map(|f| std::fs::metadata(f).and_then(|m| m.modified()).ok())
        .max();
    commit.into_iter().chain(touched).max().and_then(days_since)
}

fn size_kb(path: &Path) -> Option<u64> {
    let out = Command::new("du").args(["-sk", "--one-file-system"]).arg(path).output().ok()?;
    String::from_utf8_lossy(&out.stdout).split_whitespace().next()?.parse().ok()
}

pub fn scan(roots: &[PathBuf], idle_threshold: u64) -> Vec<Stale> {
    let mut out = Vec::new();
    for repo in repos(roots) {
        let entries = parse_porcelain(&git(&repo, &["worktree", "list", "--porcelain"]).unwrap_or_default());
        if entries.len() < 2 {
            continue;
        }
        let local = lines(git(&repo, &["for-each-ref", "--format=%(refname:short)", "refs/heads"]));
        let origin_head = git(&repo, &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"]).map(|s| s.trim().to_string());
        let default = pick_default_branch(origin_head.as_deref(), &local);
        let merged = lines(git(&repo, &["branch", "--merged", &default, "--format=%(refname:short)"]));
        let gone: Vec<String> = lines(git(&repo, &["for-each-ref", "--format=%(refname:short) %(upstream:track)", "refs/heads"]))
            .into_iter().filter(|l| l.ends_with("[gone]")).filter_map(|l| l.split(' ').next().map(str::to_string)).collect();
        for e in entries.into_iter().skip(1) {
            let path = PathBuf::from(&e.path);
            let exists = !e.prunable && path.is_dir();
            let on = |list: &[String]| e.branch.as_ref().is_some_and(|b| list.contains(b));
            let facts = Facts {
                exists,
                merged: on(&merged),
                upstream_gone: on(&gone),
                clean: exists && git(&path, &["status", "--porcelain"]).is_some_and(|s| s.trim().is_empty()),
                idle_days: if exists { idle_days(&path) } else { None },
            };
            if let Some(reason) = classify(&facts, idle_threshold) {
                out.push(Stale {
                    repo: repo.to_string_lossy().to_string(),
                    path: e.path.clone(),
                    branch: e.branch.clone(),
                    reason,
                    idle_days: facts.idle_days,
                    size_kb: if exists { size_kb(&path) } else { None },
                });
            }
        }
    }
    out
}

pub fn remove(stale: &[Stale], paths: &[String]) -> Vec<Removal> {
    paths.iter().map(|p| {
        let Some(s) = stale.iter().find(|s| &s.path == p) else {
            return Removal { path: p.clone(), ok: false, message: "Not in the last scan".into() };
        };
        let repo = Path::new(&s.repo);
        let result = if s.reason == Reason::Missing {
            Command::new("git").arg("-C").arg(repo).args(["worktree", "prune"]).output()
        } else {
            Command::new("git").arg("-C").arg(repo).args(["worktree", "remove"]).arg(&s.path).output()
        };
        let _ = Command::new("git").arg("-C").arg(repo).args(["worktree", "prune"]).output();
        match result {
            Ok(o) if o.status.success() => Removal { path: p.clone(), ok: true, message: "Removed".into() },
            Ok(o) => Removal { path: p.clone(), ok: false, message: String::from_utf8_lossy(&o.stderr).trim().to_string() },
            Err(e) => Removal { path: p.clone(), ok: false, message: e.to_string() },
        }
    }).collect()
}
```

Settings (`settings.rs`): add fields with defaults and update `Default`:

```rust
    #[serde(default = "default_worktree_roots")]
    pub worktree_roots: Vec<String>,
    #[serde(default = "default_worktree_idle_days")]
    pub worktree_idle_days: u64,
```
```rust
fn default_worktree_roots() -> Vec<String> { vec!["~/IdeaProjects".into()] }
fn default_worktree_idle_days() -> u64 { 14 }
```

`lib.rs`: `mod worktrees;`, a `LAST_SCAN: Mutex<Vec<worktrees::Stale>>` in `Shared` (`pub last_scan: Mutex<Vec<worktrees::Stale>>`, initialised empty), and commands:

```rust
fn expand_home(p: &str) -> std::path::PathBuf {
    match p.strip_prefix("~/") {
        Some(rest) => platform::home().join(rest),
        None => std::path::PathBuf::from(p),
    }
}

#[tauri::command]
async fn worktrees_scan(shared: State<'_, Shared>) -> Result<Vec<worktrees::Stale>, String> {
    let (roots, days) = {
        let s = shared.settings.lock().unwrap();
        (s.worktree_roots.iter().map(|r| expand_home(r)).collect::<Vec<_>>(), s.worktree_idle_days)
    };
    let found = tauri::async_runtime::spawn_blocking(move || worktrees::scan(&roots, days))
        .await.map_err(|e| e.to_string())?;
    *shared.last_scan.lock().unwrap() = found.clone();
    Ok(found)
}

#[tauri::command]
async fn worktrees_remove(shared: State<'_, Shared>, paths: Vec<String>) -> Result<Vec<worktrees::Removal>, String> {
    let stale = shared.last_scan.lock().unwrap().clone();
    tauri::async_runtime::spawn_blocking(move || worktrees::remove(&stale, &paths))
        .await.map_err(|e| e.to_string())
}
```
Register both. TS `Settings` type (`state.ts`) gets `worktreeRoots: string[]; worktreeIdleDays: number;` and defaults `["~/IdeaProjects"]`, `14`.

- [ ] **Step 4: View** — `layout.ts`: add `"worktrees"` and `"health"` to `IslandViewName`, layouts `worktrees: { height: 262, botX: 50, botY: 150, botDiameter: 44, agentMode: "none" }`, same for `health`. `icons.ts`: `branch: "M7 3a3 3 0 0 0-1 5.83v6.34A3 3 0 1 0 8 15.17V12.6c.9.6 2 1 3.2 1H14a3 3 0 0 0 3-3V8.83A3 3 0 1 0 15 8.83V10.6a1 1 0 0 1-1 1h-2.8A3.2 3.2 0 0 1 8 8.4V8.83A3 3 0 0 0 7 3z"`, `pulse: "M3 12h4l2.5-6 4 12 2.5-6H21v2h-3.7l-3.8 9-4-12L8.3 14H3z"`. `bridge.ts`: types from Interfaces plus

```ts
  worktreesScan: () => call<StaleWorktree[]>("worktrees_scan"),
  worktreesRemove: (paths: string[]) => call<WorktreeRemoval[]>("worktrees_remove", { paths }),
```

`windows/src/views/worktrees.ts`:

```ts
// Stale worktrees: scan, pick, remove. Nothing is selected until the user ticks it.

import { Bridge, type StaleWorktree } from "../core/bridge";
import { h, clear } from "./dom";
import type { ViewHost } from "./views";

const REASON: Record<StaleWorktree["reason"], string> = {
  missing: "folder missing", merged: "branch merged", upstreamGone: "upstream gone", idle: "idle",
};

const base = (p: string) => p.replace(/\/+$/, "").split("/").pop() ?? p;
const mb = (kb: number | null) => (kb == null ? "" : `${(kb / 1024).toFixed(kb > 10240 ? 0 : 1)} MB`);

export function buildWorktrees(blip: () => void): ViewHost {
  const list = h("div", { class: "wt-list" });
  const status = h("span", { class: "wt-status" });
  const clean = h("button", { class: "btn primary", text: "Clean selected" }) as HTMLButtonElement;
  const rescan = h("button", { class: "btn secondary", text: "Rescan" });
  const el = h("div", { class: "view" }, h("div", { class: "card" },
    h("div", { class: "wt" }, h("div", { class: "wt-head" }, h("b", { text: "Stale worktrees" }), status), list,
      h("div", { class: "wt-actions" }, rescan, clean))));
  let found: StaleWorktree[] = [];
  const selected = new Set<string>();

  function render() {
    clear(list);
    if (found.length === 0) list.append(h("div", { class: "wt-empty", text: "Nothing stale" }));
    for (const w of found) {
      const box = h("input", { type: "checkbox" }) as HTMLInputElement;
      box.checked = selected.has(w.path);
      box.onchange = () => { if (box.checked) selected.add(w.path); else selected.delete(w.path); render(); };
      const why = w.reason === "idle" && w.idleDays != null ? `idle ${w.idleDays}d` : REASON[w.reason];
      list.append(h("label", { class: "wt-row", title: w.path }, box,
        h("b", { text: base(w.repo) }), h("span", { text: w.branch ?? base(w.path) }),
        h("span", { class: "wt-why", text: why }), h("span", { class: "wt-size", text: mb(w.sizeKb) })));
    }
    clean.disabled = selected.size === 0;
    clean.textContent = selected.size ? `Clean ${selected.size}` : "Clean selected";
  }

  async function scan() {
    status.textContent = "Scanning…";
    found = (await Bridge.worktreesScan()) ?? [];
    for (const p of [...selected]) if (!found.some((w) => w.path === p)) selected.delete(p);
    status.textContent = `${found.length} found`;
    render();
  }

  rescan.addEventListener("click", () => { blip(); void scan(); });
  clean.addEventListener("click", async () => {
    blip();
    const results = (await Bridge.worktreesRemove([...selected])) ?? [];
    const failed = results.filter((r) => !r.ok);
    status.textContent = failed.length
      ? `${results.length - failed.length} removed · ${failed.length} refused: ${failed[0].message}`
      : `${results.length} removed`;
    selected.clear();
    await scan();
  });

  return { el, sync() {}, shown() { void scan(); } };
}
```

Register in `buildViews`: `map.set("worktrees", buildWorktrees(() => actions.blip()));`. Header: `tabWorktrees` with `ICONS.branch`, title "Worktrees", shown only when `State.clock` (Linux), toggled `on` for `"worktrees"`. CSS (append):

```css
.wt { display: flex; flex-direction: column; height: 100%; padding: 10px 16px 10px 96px; gap: 6px; }
.wt-head { display: flex; gap: 10px; align-items: baseline; font-size: 12.5px; }
.wt-status { color: var(--dim); font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.wt-list { flex: 1; min-height: 0; overflow-y: auto; display: flex; flex-direction: column; gap: 3px; scrollbar-width: none; }
.wt-row { display: flex; gap: 8px; align-items: center; font-size: 11.5px; padding: 4px 6px; border-radius: 8px; background: rgba(255,255,255,.04); }
.wt-row span { color: var(--dim); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.wt-why { margin-left: auto; color: var(--amber) !important; }
.wt-size { width: 56px; text-align: right; }
.wt-empty { color: var(--dim-3); font-size: 12px; }
.wt-actions { display: flex; gap: 8px; justify-content: flex-end; }
```

Settings window (`settings/main.ts`): add a "Worktrees" section with a text input bound to `worktreeRoots` (comma-separated) and a number input for `worktreeIdleDays`, saved through the existing `Bridge.saveSettings` path (follow the pattern of the existing auto-close control in that file).

- [ ] **Step 5: Push; CI green** (4 new Rust tests pass).

- [ ] **Step 6: E2E on a throwaway repo** (never the user's real worktrees):

```bash
T=~/IdeaProjects/zz-coucou-wt-test && rm -rf "$T" && git init -q -b main "$T" && cd "$T" && git commit -q --allow-empty -m init \
 && git branch merged-br && git worktree add -q ../zz-wt-merged merged-br \
 && git worktree add -q -b live-br ../zz-wt-live && touch ../zz-wt-live/dirty.txt \
 && git worktree add -q -b gone-br ../zz-wt-gone && rm -rf ../zz-wt-gone
```
Open the Worktrees tab: expect `zz-wt-merged` (branch merged) and `zz-wt-gone` (folder missing), **not** `zz-wt-live`. Tick both → Clean → `git -C "$T" worktree list` shows only the main worktree and `zz-wt-live`; `git -C "$T" branch` still lists `merged-br gone-br live-br`. Then clean up: `git -C "$T" worktree remove --force ../zz-wt-live; rm -rf "$T"`. Commit fixes.

---

### Task 5: Storage and device health

**Files:**
- Create: `windows/src-tauri/src/health.rs`, `windows/src/views/health.ts`
- Modify: `windows/src-tauri/src/lib.rs`, `windows/src/core/{bridge,state}.ts`, `windows/src/island/integrations.ts`, `windows/src/views/{views,integrations}.ts`, `windows/src/style.css`

**Interfaces:**
- Produces (Rust): `health::parse_meminfo(&str) -> MemInfo`, `health::evaluate(&Sample) -> Vec<Check>`, `health::start(app)`; event `health` with payload `HealthReport { checks: Vec<Check>, worst: Level }`; `Check { id: String, label: String, value: String, level: Level }`, `Level = "ok" | "warn" | "bad"` (serde lowercase).
- Produces (TS): `HealthReport`, `HealthCheck` types; `State.health: HealthReport | null`; system pill id `"integration_system"`; view `"health"`.

- [ ] **Step 1: Failing tests** in `health.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const MEMINFO: &str = "MemTotal:       16000000 kB\nMemFree:  100 kB\nMemAvailable:    1000000 kB\nSwapTotal:       8000000 kB\nSwapFree:        2000000 kB\n";

    #[test]
    fn parses_meminfo() {
        let m = parse_meminfo(MEMINFO);
        assert_eq!((m.total_kb, m.available_kb, m.swap_total_kb, m.swap_free_kb), (16_000_000, 1_000_000, 8_000_000, 2_000_000));
    }

    fn sample() -> Sample {
        Sample {
            disks: vec![Disk { mount: "/".into(), total: 100, free: 50 }],
            mem: Some(parse_meminfo(MEMINFO)),
            cpu_temp_c: Some(50.0),
            nvme: vec![Nvme { name: "KINGSTON".into(), critical_warnings: vec![], temp_c: Some(37.0), percent_used: Some(4) }],
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
```

- [ ] **Step 2: Push; CI `Test` fails.**

- [ ] **Step 3: Implement** `health.rs`:

```rust
// Storage and device health: disk space, NVMe SMART (UDisks2, no root), memory
// and swap, CPU temperature. Sampled every 60 s; missing sources are skipped.

use std::collections::HashMap;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level { Ok, Warn, Bad }

#[derive(Debug, Clone, Serialize)]
pub struct Check { pub id: String, pub label: String, pub value: String, pub level: Level }

#[derive(Debug, Clone, Serialize)]
pub struct HealthReport { pub checks: Vec<Check>, pub worst: Level }

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MemInfo { pub total_kb: u64, pub available_kb: u64, pub swap_total_kb: u64, pub swap_free_kb: u64 }

pub struct Disk { pub mount: String, pub total: u64, pub free: u64 }
pub struct Nvme { pub name: String, pub critical_warnings: Vec<String>, pub temp_c: Option<f64>, pub percent_used: Option<u8> }
pub struct Sample { pub disks: Vec<Disk>, pub mem: Option<MemInfo>, pub cpu_temp_c: Option<f64>, pub nvme: Vec<Nvme> }

pub fn parse_meminfo(text: &str) -> MemInfo {
    let mut m = MemInfo::default();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let (Some(key), Some(v)) = (it.next(), it.next().and_then(|v| v.parse::<u64>().ok())) else { continue };
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

fn pct(part: u64, whole: u64) -> f64 { if whole == 0 { 0.0 } else { part as f64 * 100.0 / whole as f64 } }

pub fn evaluate(s: &Sample) -> Vec<Check> {
    let mut out = Vec::new();
    for d in &s.disks {
        let free = pct(d.free, d.total);
        out.push(Check {
            id: format!("disk:{}", d.mount), label: format!("Disk {}", d.mount),
            value: format!("{:.0}% free · {:.0} GB", free, d.free as f64 / 1e9),
            level: if free < 10.0 { Level::Bad } else if free < 20.0 { Level::Warn } else { Level::Ok },
        });
    }
    if let Some(m) = &s.mem {
        let avail = pct(m.available_kb, m.total_kb);
        out.push(Check { id: "memory".into(), label: "Memory".into(), value: format!("{avail:.0}% available"),
            level: if avail < 10.0 { Level::Warn } else { Level::Ok } });
        if m.swap_total_kb > 0 {
            let used = pct(m.swap_total_kb - m.swap_free_kb, m.swap_total_kb);
            out.push(Check { id: "swap".into(), label: "Swap".into(), value: format!("{used:.0}% used"),
                level: if used > 50.0 { Level::Warn } else { Level::Ok } });
        }
    }
    if let Some(t) = s.cpu_temp_c {
        out.push(Check { id: "cpu".into(), label: "CPU temperature".into(), value: format!("{t:.0} °C"),
            level: if t > 90.0 { Level::Bad } else if t > 80.0 { Level::Warn } else { Level::Ok } });
    }
    for n in &s.nvme {
        let hot = n.temp_c.is_some_and(|t| t > 70.0);
        let worn = n.percent_used.is_some_and(|p| p > 90);
        let mut value = Vec::new();
        if let Some(p) = n.percent_used { value.push(format!("{p}% worn")); }
        if let Some(t) = n.temp_c { value.push(format!("{t:.0} °C")); }
        if !n.critical_warnings.is_empty() { value.push(format!("warning: {}", n.critical_warnings.join(", "))); }
        out.push(Check { id: format!("nvme:{}", n.name), label: format!("NVMe {}", n.name), value: value.join(" · "),
            level: if !n.critical_warnings.is_empty() || worn || hot { Level::Bad } else { Level::Ok } });
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
        let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
        // SAFETY: statvfs only writes into the struct we own.
        if unsafe { libc::statvfs(path.as_ptr(), &mut st) } != 0 || seen.contains(&st.f_fsid) {
            continue; // same filesystem as `/` → one row, not two
        }
        seen.push(st.f_fsid);
        let frsize = st.f_frsize as u64;
        out.push(Disk { mount: mount.into(), total: st.f_blocks as u64 * frsize, free: st.f_bavail as u64 * frsize });
    }
    out
}

fn cpu_temp() -> Option<f64> {
    for dir in std::fs::read_dir("/sys/class/hwmon").ok()?.flatten() {
        if std::fs::read_to_string(dir.path().join("name")).ok()?.trim() != "coretemp" { continue }
        let max = std::fs::read_dir(dir.path()).ok()?.flatten()
            .filter(|f| f.file_name().to_string_lossy().ends_with("_input"))
            .filter_map(|f| std::fs::read_to_string(f.path()).ok()?.trim().parse::<f64>().ok())
            .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))));
        return max.map(|v| v / 1000.0);
    }
    None
}

async fn nvme() -> Vec<Nvme> {
    let Ok(conn) = zbus::Connection::system().await else { return vec![] };
    let Ok(om) = zbus::fdo::ObjectManagerProxy::builder(&conn)
        .destination("org.freedesktop.UDisks2").and_then(|b| b.path("/org/freedesktop/UDisks2"))
        .map(|b| b.build()) else { return vec![] };
    let Ok(om) = om.await else { return vec![] };
    let Ok(objects) = om.get_managed_objects().await else { return vec![] };
    let mut out = Vec::new();
    for (path, ifaces) in objects {
        let Some(ctrl) = ifaces.get("org.freedesktop.UDisks2.NVMe.Controller") else { continue };
        let name = path.as_str().rsplit('/').next().unwrap_or("nvme").split('_').next().unwrap_or("nvme").to_string();
        let warnings: Vec<String> = ctrl.get("SmartCriticalWarning")
            .and_then(|v| Vec::<String>::try_from(v.try_clone().ok()?).ok()).unwrap_or_default();
        let temp_c = ctrl.get("SmartTemperature").and_then(|v| u16::try_from(v).ok())
            .filter(|k| *k > 0).map(|k| k as f64 - 273.15);
        let percent_used = smart_percent_used(&conn, &path).await;
        out.push(Nvme { name, critical_warnings: warnings, temp_c, percent_used });
    }
    out
}

async fn smart_percent_used(conn: &zbus::Connection, path: &zbus::zvariant::OwnedObjectPath) -> Option<u8> {
    use zbus::zvariant::OwnedValue;
    let reply = conn.call_method(Some("org.freedesktop.UDisks2"), path.as_str(),
        Some("org.freedesktop.UDisks2.NVMe.Controller"), "SmartGetAttributes",
        &(HashMap::<String, zbus::zvariant::Value>::new(),)).await.ok()?;
    let attrs: HashMap<String, OwnedValue> = reply.body().deserialize().ok()?;
    u8::try_from(attrs.get("percent_used")?.try_clone().ok()?).ok()
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
            if crate::integrations::PAUSED.load(std::sync::atomic::Ordering::Relaxed) { continue }
            let checks = evaluate(&sample().await);
            let worst = worst(&checks);
            let _ = app.emit_to(crate::island::WINDOW_LABEL, "health", HealthReport { checks, worst });
        }
    });
}
```
In `lib.rs`: `#[cfg(target_os = "linux")] mod health;` and in `setup` after `integrations::start(...)`: `#[cfg(target_os = "linux")] health::start(handle.clone());`. The health tests are Linux-only by the module cfg, which is where CI runs.

- [ ] **Step 4: TS.** `bridge.ts`: `export interface HealthCheck { id: string; label: string; value: string; level: "ok" | "warn" | "bad" }`, `export interface HealthReport { checks: HealthCheck[]; worst: "ok" | "warn" | "bad" }`. `state.ts`: `health: HealthReport | null = null;` and append to `INTEGRATION_AGENTS`: `task("integration_system", "System", "#22C55E", "n8n")`; in `loadIntegrationTasks`, load it when `this.clock` is set (`proto.id === "integration_system" ? this.clock != null : …`). In `island/integrations.ts` `registerIntegrationHandlers`:

```ts
  void onEvent<HealthReport>("health", (report) => {
    const was = State.health?.worst ?? "ok";
    State.health = report;
    const task = State.tasks.find((t) => t.id === "integration_system");
    if (task) {
      task.color = { ok: "#22C55E", warn: "#F5A524", bad: "#F4505E" }[report.worst];
      task.state = report.worst === "bad" ? "error" : "idle";
      // Once per transition into red, never on every sample.
      if (report.worst === "bad" && was !== "bad") {
        if (State.focusId !== task.id) task.pillBadge = "error";
        Sound.play("error");
        island.reveal();
      }
    }
    State.notify();
  });
```

`windows/src/views/health.ts`:

```ts
// Health view: one row per check, coloured by level.

import { State } from "../core/state";
import { h, clear, dot } from "./dom";
import type { ViewHost } from "./views";

const COLOR = { ok: "#22C55E", warn: "#F5A524", bad: "#F4505E" } as const;

export function buildHealth(): ViewHost {
  const list = h("div", { class: "health" });
  const el = h("div", { class: "view" }, h("div", { class: "card" }, list));
  return {
    el,
    sync() {
      clear(list);
      const report = State.health;
      if (!report) { list.append(h("div", { class: "wt-empty", text: "Checking…" })); return; }
      for (const c of report.checks) {
        list.append(h("div", { class: "health-row" }, dot(COLOR[c.level], 8),
          h("b", { text: c.label }), h("span", { text: c.value })));
      }
    },
  };
}
```
Register `map.set("health", buildHealth());`, header tab `ICONS.pulse` (Linux only), and in `renderIntegrationCard`/pill click for `integration_system` open the health view (`actions.setView("health")` from `buildPill`'s onclick when `task.id === "integration_system"`). CSS:

```css
.health { display: flex; flex-direction: column; gap: 5px; padding: 12px 16px 12px 96px; }
.health-row { display: flex; align-items: center; gap: 9px; font-size: 12px; padding: 5px 9px; border-radius: 9px; background: rgba(255,255,255,.04); }
.health-row span { margin-left: auto; color: var(--dim); }
```

- [ ] **Step 5: Push; CI green** (4 health tests).

- [ ] **Step 6: E2E.** Install; wait ≤ 60 s; open Health tab → rows for `Disk /` (and `/home` only if separate fs), `Memory`, `Swap`, `CPU temperature`, `NVMe KINGSTON` with a temperature near `busctl get-property org.freedesktop.UDisks2 <drive> org.freedesktop.UDisks2.NVMe.Controller SmartTemperature` − 273. Compare disk % with `df -h /`. Commit fixes.

---

### Task 6: Full regression pass, docs, install

**Files:** Modify `windows/README.md` (Linux section: popup behaviour, sessions, IntelliJ, worktrees, health), `docs/superpowers/specs/...` untouched.

- [ ] **Step 1:** Run `windows/dev/linux-e2e/e2e.sh 3` on the final artifact (flat accel; restore after). Expected: all PASS, including focus/Esc checks.
- [ ] **Step 2:** Scripted alert check: send a fake `PermissionRequest` through the socket while a terminal is focused — `printf '%s\n' '{"hook_event_name":"PermissionRequest","request_id":"t1","session_id":"x","cwd":"/tmp","tool_name":"Bash","tool_input":{"command":"echo hi"}}' | ~/.local/share/coucou/bin/coucou-hook PermissionRequest` (check `hook/src/main.rs` for the exact invocation first). Expect: island shows the approval, `_NET_ACTIVE_WINDOW` unchanged (no focus steal). Deny it via vmouse click on "Deny".
- [ ] **Step 3:** Update README Linux section; commit; final CI; copy the `.deb` to `~/Downloads`; tell the user the `sudo apt install` command.

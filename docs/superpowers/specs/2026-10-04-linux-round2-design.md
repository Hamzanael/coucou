# Coucou for Linux — round 2: typing, closing, sessions, worktrees, health

Date: 2026-10-04 · Platform: Linux (Ubuntu 24.04, GNOME 46, Wayland + XWayland) · Branch: `claude/topbar-clock-calendar`

## What the user asked for

1. The chat field ("Ask me anything…") does not accept typing.
2. Claude Code sessions are not visible.
3. "Open in VS Code" should open the project in IntelliJ instead.
4. Clean stale git worktrees.
5. Check storage and device health.
6. The open island cannot be minimised — wants a close button **and** close-on-click-elsewhere.

## Root cause behind 1 and 6 (verified)

The island is an **override-redirect** X window so it can sit over GNOME's top bar.
GNOME Shell never gives keyboard focus to an override-redirect window: X reports
`FocusIn`, but every keystroke still goes to the focused Wayland window (tested with a
virtual keyboard). So the chat can't be typed into, `Esc` never arrives, and with no
focus there is no focus-out to tell us the user clicked elsewhere.

## Design

### A. Two window modes (fixes 1 and 6)

| Island state | Window | Where |
|---|---|---|
| Clock pill, compact | override-redirect (as now) | in the top bar, over the clock |
| Expanded (calendar, overview, chat, health, worktrees, approvals…) | **managed** popup: undecorated, keep-above, skip taskbar/pager | just **below** the top bar, centred — like GNOME's own clock menu |

- Switching mode = unmap → toggle override-redirect → map (one frame; the island already animates there).
- Opening by **click** focuses the popup (using the click's timestamp so GNOME's focus-stealing prevention allows it). Then typing works, `Esc` closes, and **focus-out = clicked elsewhere → close**.
- Opening by an **alert** (approval, error) maps it *without* taking focus, so it never steals your typing; it closes via its buttons, the close button or the auto-close timer.
- New **close button** (×) at the right of the header → back to the clock pill at once.

### B. Claude Code sessions (2)

- Track sessions by `session_id` from the hook payload, instead of one merged "VS Code" entry.
- Overview lists one row per live session: project name (cwd basename), state (thinking / working / waiting for approval / done / error), latest step, time since last event.
- A session disappears on `SessionEnd`, or after 30 min with no events.
- Step labels in English (they are French today: "Exécute", "Lit"…).
- Approvals stay one card at a time, now naming the session's project.
- The compact island's Mochi reflects the busiest session.

### C. IntelliJ (3)

- "Open terminal / VS Code" becomes **Open in IntelliJ**: runs `idea <cwd>` via `~/.local/share/JetBrains/Toolbox/scripts/idea` (falls back to `idea` on PATH; if neither exists the button says so).
- Each session row has the same action for its own project.

### D. Stale worktrees (4)

- Scans git repos directly under `~/IdeaProjects` (configurable list of roots in Settings).
- For each `git worktree list --porcelain` entry (main worktree excluded), it is **stale** when any of:
  - **missing** — the directory is gone (git marks it prunable);
  - **merged / gone** — its branch is merged into the repo's default branch, or its upstream shows `[gone]` (local refs only — no `git fetch`, no network);
  - **idle** — `git status --porcelain` is clean **and** nothing in it changed for 14 days (configurable).
- New **Worktrees** view: candidates grouped by repo, each with its reason and size; nothing is pre-selected.
- **Clean** runs, per selected entry, `git worktree remove <path>` (never `--force`: a dirty worktree is refused and reported) and then `git worktree prune`. **Branches are never deleted.**
- Nothing is removed without an explicit click.

### E. Storage & device health (5)

New Rust module `health.rs`, sampled every 60 s only while the island is open or the pill is shown (0 % CPU when hidden stays true):

| Check | Source (no root) | Warn when |
|---|---|---|
| Disk space `/` and `/home` | `statvfs` | < 10 % free |
| NVMe health (Kingston SNV3S 500G) | UDisks2 `NVMe.Controller`: `SmartCriticalWarning`, `SmartTemperature`, `SmartGetAttributes` → percent used, spare | any critical warning, > 90 % used, > 70 °C |
| Memory + swap | `/proc/meminfo` | available < 10 %, swap > 50 % used |
| CPU temperature | hwmon `coretemp` | > 90 °C |

- A **System** pill (mini Mochi: green / amber / red) next to GitHub; new **Health** view with each check's value.
- A red result reveals the island once with a badge; it does not keep nagging.

## Order of work

1. A (typing, Esc, close button, click-elsewhere) — the blocker.
2. B + C (sessions, IntelliJ).
3. D (worktrees).
4. E (health).

Each step is built in the fork's CI and verified end to end on the real desktop with the
uinput mouse harness. Keyboard checks run only after confirming via `_NET_ACTIVE_WINDOW`
that the test window holds focus, so no test keystroke can reach another window.

## Out of scope

Windows build changes beyond keeping it compiling; deleting branches; fetching remotes;
SMART for non-NVMe disks; battery (none detected).

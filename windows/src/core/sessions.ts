// Claude Code sessions, one per `session_id`, built from hook events. Pure: the
// island feeds it events and reads rows; nothing here touches the DOM or Tauri.

export interface HookPayload {
  hook_event_name?: string;
  request_id?: string;
  session_id?: string;
  cwd?: string;
  message?: string;
  /** UserPromptSubmit carries `prompt`; `message` belongs to Notification/Stop. */
  prompt?: string;
  tool_name?: string;
  tool_input?: Record<string, unknown>;
}

export type SessionState = "idle" | "thinking" | "working" | "approval" | "question" | "finished" | "error";

export interface Session {
  id: string;
  project: string;
  cwd: string;
  state: SessionState;
  step: string;
  updatedAt: number;
  /** Known to be running (from Claude Code's own records): exempt from the TTL. */
  live?: boolean;
}

/** A running session as Rust reads it from ~/.claude/sessions. */
export interface LiveSession {
  sessionId: string;
  cwd: string;
  busy: boolean;
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
  const file = str("file_path") ?? str("path");
  const detail =
    str("command")?.slice(0, 40) ??
    (file ? lastPathComponent(file) : null) ??
    str("query")?.slice(0, 40) ??
    str("pattern")?.slice(0, 40);
  return detail ? `${label} · ${detail}` : label;
}

/** Which session state wins when several run at once — the one needing you. */
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
    const cwd = p.cwd ?? "";
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

  /**
   * The running sessions are the truth for which rows exist: quiet ones are
   * added, ended ones dropped. Hook events keep supplying the detail.
   */
  sync(live: LiveSession[], now: number) {
    const running = new Set(live.map((l) => l.sessionId));
    for (const id of [...this.sessions.keys()]) if (!running.has(id)) this.sessions.delete(id);
    for (const l of live) {
      const s = this.sessions.get(l.sessionId);
      if (!s) {
        this.sessions.set(l.sessionId, {
          id: l.sessionId, cwd: l.cwd, project: lastPathComponent(l.cwd) || "Session",
          state: l.busy ? "working" : "idle", step: "", updatedAt: Math.min(l.updatedAt, now), live: true,
        });
        continue;
      }
      s.live = true;
      if (!l.busy && (s.state === "working" || s.state === "thinking")) s.state = "idle";
      if (l.busy && s.state === "idle") s.state = "working";
      s.updatedAt = Math.max(s.updatedAt, Math.min(l.updatedAt, now));
    }
  }

  list(now: number): Session[] {
    for (const [id, s] of this.sessions) {
      if (!s.live && now - s.updatedAt > SESSION_TTL_MS) this.sessions.delete(id);
    }
    return [...this.sessions.values()].sort((a, b) => b.updatedAt - a.updatedAt);
  }

  busiest(now: number): SessionState | "idle" {
    const states = new Set(this.list(now).map((s) => s.state));
    return PRIORITY.find((p) => states.has(p)) ?? "idle";
  }
}

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

  it("syncs with the running sessions: adds quiet ones, drops ended ones, keeps hook detail", () => {
    const s = new SessionStore();
    s.apply(ev("PreToolUse", { tool_name: "Bash", tool_input: { command: "ls" } }), 10);
    s.apply(ev("PreToolUse", { session_id: "gone", tool_name: "Read", tool_input: {} }), 10);
    s.sync(
      [
        { sessionId: "s1", cwd: "/home/u/IdeaProjects/analytickBE", busy: true, updatedAt: 5 },
        { sessionId: "q", cwd: "/home/u/IdeaProjects/msk", busy: false, updatedAt: 3 },
      ],
      20,
    );
    const rows = s.list(20);
    expect(rows.map((r) => r.id)).toEqual(["s1", "q"]);
    expect(rows[0]).toMatchObject({ state: "working", step: "Run · ls" });
    expect(rows[1]).toMatchObject({ project: "msk", state: "idle" });
  });

  it("a running session that went idle stops showing as working, but keeps approvals", () => {
    const s = new SessionStore();
    s.apply(ev("PreToolUse", { tool_name: "Bash", tool_input: {} }), 1);
    s.apply(ev("PermissionRequest", { session_id: "p", tool_name: "Bash", tool_input: {} }), 1);
    s.sync(
      [
        { sessionId: "s1", cwd: "/a", busy: false, updatedAt: 2 },
        { sessionId: "p", cwd: "/b", busy: false, updatedAt: 2 },
      ],
      3,
    );
    expect(s.list(3).find((r) => r.id === "s1")?.state).toBe("idle");
    expect(s.list(3).find((r) => r.id === "p")?.state).toBe("approval");
    expect(s.busiest(3)).toBe("approval");
  });

  it("keeps the last 8 steps per session for the detail card", () => {
    const s = new SessionStore();
    for (let i = 0; i < 10; i++) s.apply(ev("PreToolUse", { tool_name: "Bash", tool_input: { command: `c${i}` } }), i);
    const row = s.list(10)[0];
    expect(row.steps).toHaveLength(8);
    expect(row.steps[7]).toBe("Run · c9");
    expect(row.steps[0]).toBe("Run · c2");
  });

  it("labels steps in English", () => {
    expect(stepLabel("Edit", { file_path: "/x/y/Main.kt" })).toBe("Edit · Main.kt");
    expect(stepLabel("Grep", { pattern: "foo" })).toBe("Search · foo");
    expect(stepLabel("Mystery", {})).toBe("Mystery");
  });
});

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

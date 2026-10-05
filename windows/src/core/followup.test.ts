import { describe, expect, it } from "vitest";
import { followUp, nudgeCandidates, STALE_MS } from "./followup";
import { SessionStore, type Session } from "./sessions";

const base = (over: Partial<Session> = {}): Session => ({
  id: "s", project: "p", cwd: "/p", state: "idle", step: "", steps: [], updatedAt: 0, since: 0, ...over,
});

describe("followUp", () => {
  it("flags a session waiting on you", () => {
    expect(followUp(base({ state: "approval" }), 1)).toBe("waiting");
    expect(followUp(base({ state: "question" }), 1)).toBe("waiting");
    expect(followUp(base({ state: "finished", lastReply: "Should I push it?" }), 1)).toBe("waiting");
  });

  it("flags errored or interrupted turns as unfinished", () => {
    expect(followUp(base({ state: "error" }), 1)).toBe("unfinished");
    expect(followUp(base({ state: "idle", interrupted: true }), 1)).toBe("unfinished");
  });

  it("turns either into stale after 30 minutes of silence", () => {
    expect(followUp(base({ state: "error", updatedAt: 0 }), STALE_MS)).toBe("stale");
    expect(followUp(base({ state: "question", updatedAt: 0 }), STALE_MS + 1)).toBe("stale");
  });

  it("leaves a cleanly finished or working session alone", () => {
    expect(followUp(base({ state: "finished", lastReply: "Done, all tests pass." }), 1)).toBeNull();
    expect(followUp(base({ state: "working" }), STALE_MS * 2)).toBeNull();
    expect(followUp(base({ state: "idle" }), STALE_MS * 2)).toBeNull();
  });
});

describe("SessionStore interruption", () => {
  it("marks a turn that stopped without a Stop event as interrupted, and clears it on the next prompt", () => {
    const s = new SessionStore();
    s.apply({ hook_event_name: "PreToolUse", session_id: "a", cwd: "/p", tool_name: "Bash", tool_input: {} }, 1);
    s.sync([{ sessionId: "a", cwd: "/p", busy: false, updatedAt: 1 }], 2);
    expect(s.list(2)[0].interrupted).toBe(true);
    s.apply({ hook_event_name: "UserPromptSubmit", session_id: "a", prompt: "go on" }, 3);
    expect(s.list(3)[0].interrupted).toBeFalsy();
  });
});

describe("nudgeCandidates", () => {
  it("auto-continues only stale unfinished sessions, once each, never ones waiting on a question", () => {
    const stale = STALE_MS + 1;
    const sessions = [
      base({ id: "err", state: "error", pid: 1 }),
      base({ id: "ask", state: "question", pid: 2 }),
      base({ id: "cut", state: "idle", interrupted: true, pid: 3 }),
      base({ id: "fresh", state: "error", updatedAt: stale - 10, pid: 4 }),
    ];
    expect(nudgeCandidates(sessions, new Set(), stale).map((s) => s.id)).toEqual(["err", "cut"]);
    expect(nudgeCandidates(sessions, new Set(["err"]), stale).map((s) => s.id)).toEqual(["cut"]);
  });
});

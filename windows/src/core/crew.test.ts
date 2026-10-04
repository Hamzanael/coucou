import { describe, expect, it } from "vitest";
import { dashboardSize, nextTab, sessionAgent } from "./crew";
import type { Session } from "./sessions";

const session = (over: Partial<Session> = {}): Session => ({
  id: "abc", project: "analytickBE", cwd: "/x/analytickBE", state: "working", step: "Run · ls",
  steps: ["Run · ls"], updatedAt: 1, since: 1, ...over,
});

describe("sessionAgent", () => {
  it("turns a session into a crew member with a stable per-project colour", () => {
    const a = sessionAgent(session());
    expect(a.id).toBe("session:abc");
    expect(a.name).toBe("analytickBE");
    expect(a.state).toBe("working");
    expect(sessionAgent(session({ id: "other" })).color).toBe(a.color);
    expect(a.color).toMatch(/^#[0-9A-F]{6}$/i);
  });

  it("shows approvals and errors as the bot's own alarm states", () => {
    expect(sessionAgent(session({ state: "approval" })).state).toBe("approval");
    expect(sessionAgent(session({ state: "idle" })).state).toBe("idle");
  });
});

describe("nextTab", () => {
  const tabs = ["overview", "prompt", "calendar"] as const;
  it("cycles both ways and wraps", () => {
    expect(nextTab([...tabs], "overview", 1)).toBe("prompt");
    expect(nextTab([...tabs], "calendar", 1)).toBe("overview");
    expect(nextTab([...tabs], "overview", -1)).toBe("calendar");
  });
  it("starts from the first tab when the current view is not a tab", () => {
    expect(nextTab([...tabs], "approval", 1)).toBe("overview");
  });
});

describe("dashboardSize", () => {
  it("is 1200×800 when the screen allows, never wider/taller than the screen minus margins", () => {
    expect(dashboardSize(2560, 1440)).toEqual({ w: 1200, h: 800 });
    expect(dashboardSize(1366, 768)).toEqual({ w: 1200, h: 648 });
    expect(dashboardSize(1000, 600)).toEqual({ w: 920, h: 480 });
  });
});

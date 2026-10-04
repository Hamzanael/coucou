// Expand mode: everything at once — the crew with each session's live steps,
// pipelines, health, calendar and worktrees — in a large window under the top
// bar. Panels are the same builders as the tabs, in their own instances.

import { Bridge } from "../core/bridge";
import { sessionAgent } from "../core/crew";
import type { Session } from "../core/sessions";
import { createMiniBot, pruneMiniBots } from "../mochi/minibots";
import { buildCalendar } from "./calendar";
import { h, clear } from "./dom";
import { buildHealth } from "./health";
import { timeAgo } from "./integrations";
import { buildPipelines } from "./pipelines";
import { liveSessions } from "./sessions";
import { renderTeleport } from "./teleport";
import type { ViewHost } from "./views";
import { buildWorktrees } from "./worktrees";

function sessionCard(s: Session): HTMLElement {
  const agent = sessionAgent(s);
  const steps = h("div", { class: "dash-steps" });
  for (const step of s.steps.slice(-4)) steps.append(h("div", { text: step }));
  return h(
    "div",
    { class: "dash-session", style: `--accent:${agent.color}` },
    createMiniBot(agent, 30),
    h(
      "div",
      { class: "dash-session-body" },
      h("div", { class: "dash-session-head" }, h("b", { text: s.name || s.project }),
        h("span", { text: `${s.name ? `${s.project} · ` : ""}${s.state} · ${timeAgo(s.updatedAt)}` })),
      steps,
      h("button", {
        class: "link-btn",
        "data-nav": true,
        style: `color:${agent.color}`,
        text: "Open in IntelliJ",
        title: s.cwd,
        onclick: () => void Bridge.openInIde(s.cwd || null),
      }),
    ),
  );
}

function panel(title: string, host: ViewHost): HTMLElement {
  host.el.classList.add("on");
  return h("section", { class: "dash-panel" }, h("h3", { text: title }), host.el);
}

export function buildDashboard(blip: () => void): ViewHost {
  const sessions = h("div", { class: "dash-sessions" });
  const children: ViewHost[] = [
    buildPipelines(),
    buildHealth(blip),
    buildCalendar(blip),
    buildWorktrees(blip),
  ];
  const [pipes, health, calendar, worktrees] = children;
  const el = h(
    "div",
    { class: "view dash" },
    h("section", { class: "dash-panel dash-crew" }, h("h3", { text: "Claude Code sessions" }), sessions),
    panel("Pipelines", pipes),
    panel("Health", health),
    panel("Calendar", calendar),
    panel("Worktrees", worktrees),
  );
  let key = "";

  return {
    el,
    sync() {
      const list = liveSessions();
      const next = list.map((s) => `${s.id}${s.state}${s.steps.join("|")}${Math.floor(s.updatedAt / 60_000)}`).join("#");
      if (next !== key) {
        key = next;
        clear(sessions);
        for (const s of list) sessions.append(sessionCard(s));
        sessions.append(renderTeleport());
        pruneMiniBots();
      }
      for (const c of children) c.sync();
    },
    shown() {
      key = "";
      for (const c of children) c.shown?.();
    },
  };
}

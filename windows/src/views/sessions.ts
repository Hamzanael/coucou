// Claude Code sessions in the overview: every session is an avatar in the crew
// (right card); the selected one — the most recent by default — gets the left
// card with its latest steps and an "Open in IntelliJ" button.

import { Bridge } from "../core/bridge";
import { SESSION_PREFIX, sessionAgent } from "../core/crew";
import { State, type AgentTask, type PillBadge } from "../core/state";
import type { Session } from "../core/sessions";
import { timeAgo } from "./integrations";
import { h, dot } from "./dom";

const STATE_LABEL: Record<Session["state"], string> = {
  idle: "idle",
  thinking: "thinking",
  working: "working",
  approval: "needs approval",
  question: "waiting for you",
  finished: "done",
  error: "error",
};

const STATE_BADGE: Partial<Record<Session["state"], PillBadge>> = {
  approval: "approval",
  question: "approval",
  error: "error",
  finished: "finished",
};

export const liveSessions = () => State.sessions.list(Date.now());

/** The session the left card shows: the one picked in the crew, else the latest. */
export function selectedSession(sessions: Session[]): Session | null {
  return sessions.find((s) => s.id === State.focusSessionId) ?? sessions[0] ?? null;
}

/** Every session as an avatar, then the integration pills (not the Claude one). */
export function crew(sessions: Session[]): AgentTask[] {
  const avatars = sessions.map((s) => ({ ...sessionAgent(s), pillBadge: STATE_BADGE[s.state] ?? null }));
  const integrations = State.tasks.filter((t) => t.id !== "integration_claude" && t.id !== State.focusId);
  return [...avatars, ...integrations];
}

export const isSessionAgent = (task: AgentTask) => task.id.startsWith(SESSION_PREFIX);

export function renderSessionDetail(s: Session): HTMLElement {
  const color = sessionAgent(s).color;
  const steps = h("div", { class: "session-steps" });
  const recent = s.steps.slice(-4);
  recent.forEach((step, i) => {
    steps.append(h("div", { class: i === recent.length - 1 ? "session-step now" : "session-step", text: step }));
  });
  if (recent.length === 0) steps.append(h("div", { class: "session-step", text: STATE_LABEL[s.state] }));

  return h(
    "div",
    { class: "session-card" },
    h(
      "div",
      { class: "session-head" },
      dot(color, 7),
      h("b", { text: s.name || s.project }),
      h("span", { class: "session-state", text: `${s.name ? `${s.project} · ` : ""}${STATE_LABEL[s.state]} · ${timeAgo(s.updatedAt)}` }),
    ),
    steps,
    h("button", {
      class: "link-btn session-open",
      "data-nav": true,
      style: `color:${color}`,
      text: "Open in IntelliJ",
      title: s.cwd,
      onclick: () => void Bridge.openInIde(s.cwd || null),
    }),
  );
}

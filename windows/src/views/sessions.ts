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

/** The short state shown next to a session in the crew list. */
export const stateLabel = (state: Session["state"]) => STATE_LABEL[state];

/** What the session is about and where it is: your ask, Claude's latest word, what it is doing. */
export function renderSessionDetail(s: Session): HTMLElement {
  const color = sessionAgent(s).color;
  const lines = h("div", { class: "session-lines" });
  const line = (who: string, text: string | undefined, cls = "") => {
    if (text) lines.append(h("div", { class: `session-line ${cls}` }, h("i", { text: who }), h("span", { text })));
  };
  line("You", s.lastPrompt);
  line("Claude", s.lastReply);
  if (s.state === "working" || s.state === "approval" || s.state === "question") line("Now", s.step, "now");

  return h(
    "div",
    { class: "session-card" },
    h("div", { class: "session-head" }, dot(color, 8), h("b", { text: s.title || s.name || s.project })),
    h("div", { class: "session-meta", text: `${s.project} · ${STATE_LABEL[s.state]} · ${timeAgo(s.updatedAt)}` }),
    lines,
    sessionButtons(s, color),
  );
}

/** Terminal (the Ghostty tab running it) and IntelliJ (its folder). */
export function sessionButtons(s: Session, color: string): HTMLElement {
  const row = h("div", { class: "session-buttons" });
  if (s.pid) {
    row.append(
      h("button", {
        class: "link-btn",
        "data-nav": true,
        style: `color:${color}`,
        text: "Terminal",
        title: "Go to the terminal running this session",
        onclick: () => void goToTerminal(s),
      }),
    );
  }
  row.append(
    h("button", {
      class: "link-btn",
      "data-nav": true,
      style: `color:${color}b3`,
      text: "IntelliJ",
      title: s.cwd,
      onclick: () => void Bridge.openInIde(s.cwd || null),
    }),
  );
  return row;
}

export async function goToTerminal(s: Session) {
  if (!s.pid) return;
  try {
    await Bridge.focusTerminal(s.pid);
  } catch (err) {
    State.noteMessage = String(err).replace(/^Error:\s*/, "");
    void Bridge.log(`terminal: ${State.noteMessage}`);
  }
}

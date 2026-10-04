// The overview's session list: one row per live Claude Code session.

import { Bridge } from "../core/bridge";
import { State } from "../core/state";
import type { Session } from "../core/sessions";
import { timeAgo } from "./integrations";
import { h, dot } from "./dom";

const STATE_COLOR: Record<Session["state"], string> = {
  thinking: "#A78BFA",
  working: "#3B9EFF",
  approval: "#F5A524",
  question: "#22D3EE",
  finished: "#34D399",
  error: "#F4505E",
};

/** What fits the overview's left card. */
const MAX_ROWS = 3;

export function renderSessions(sessions: Session[]): HTMLElement {
  const list = h("div", { class: "sessions" });
  for (const s of sessions.slice(0, MAX_ROWS)) {
    list.append(
      h(
        "button",
        {
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
  if (sessions.length > MAX_ROWS) {
    list.append(h("div", { class: "session-more", text: `+${sessions.length - MAX_ROWS} more` }));
  }
  return list;
}

export const liveSessions = () => State.sessions.list(Date.now());

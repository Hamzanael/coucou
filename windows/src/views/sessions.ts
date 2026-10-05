// Claude Code sessions in the overview: every session is an avatar in the crew
// (right card); the selected one — the most recent by default — gets the left
// card with its latest steps and an "Open in IntelliJ" button.

import { Bridge } from "../core/bridge";
import { SESSION_PREFIX, sessionAgent } from "../core/crew";
import { followUp } from "../core/followup";
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

let lastShown: string | null = null;

/**
 * The session the left card shows: the one picked in the crew, else the one it
 * showed last (so it doesn't jump on every update), else the most active.
 */
export function selectedSession(sessions: Session[]): Session | null {
  const picked =
    sessions.find((s) => s.id === State.focusSessionId) ??
    sessions.find((s) => s.id === lastShown) ??
    [...sessions].sort((a, b) => b.updatedAt - a.updatedAt)[0] ??
    null;
  lastShown = picked?.id ?? null;
  return picked;
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

/**
 * What the session is about and where it is: your ask, Claude's latest word,
 * what it is doing. One card, updated in place — its buttons must stay
 * clickable while the session works and its text changes every second.
 */
export class SessionCard {
  readonly el: HTMLElement;
  private dotEl = dot("#fff", 8);
  private titleEl = h("b");
  private metaEl = h("div", { class: "session-meta" });
  private you = SessionCard.line("You");
  private claude = SessionCard.line("Claude");
  private now = SessionCard.line("Now", "now");
  private nowLabel = this.now.row.firstElementChild as HTMLElement;
  private buttons: HTMLElement = h("div");
  private buttonsKey = "";
  private session: Session | null = null;
  private replies = h("div", { class: "session-replies" });
  private dialogHint = h("div", { class: "session-hint", text: "Multiple-choice question — answer it in the terminal" });
  private input = h("input", { type: "text", class: "session-input", placeholder: "Reply to Claude…" }) as HTMLInputElement;
  private status = h("span", { class: "session-status" });

  constructor(private requestKeyboard: () => void) {
    const quick = (label: string, text: string) =>
      h("button", { class: "reply-chip", "data-nav": true, text: label, onclick: () => void this.send(text) });
    const replyBtn = h("button", {
      class: "reply-chip",
      "data-nav": true,
      text: "Reply…",
      onclick: () => {
        this.input.style.display = "";
        // Typing needs the keyboard, which GNOME gives only below the top bar.
        this.requestKeyboard();
        window.setTimeout(() => this.input.focus(), 180);
      },
    });
    this.input.style.display = "none";
    this.input.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && this.input.value.trim()) {
        e.preventDefault();
        void this.send(this.input.value.trim()).then(() => (this.input.value = ""));
      }
    });
    this.replies.append(
      quick("Continue", "Please continue."),
      quick("Yes", "Yes."),
      quick("No", "No."),
      replyBtn,
      this.status,
    );
    this.el = h(
      "div",
      { class: "session-card" },
      h("div", { class: "session-head" }, this.dotEl, this.titleEl),
      this.metaEl,
      h("div", { class: "session-lines" }, this.you.row, this.claude.row, this.now.row),
      this.dialogHint,
      this.replies,
      this.input,
      this.buttons,
    );
  }

  /** Into the session's inbox, labelled as coming from you through Coucou. */
  private async send(text: string) {
    const s = this.session;
    if (!s) return;
    this.status.textContent = "Sending…";
    try {
      await Bridge.sendToSession(s.id, `From Hamza via Coucou: ${text}`);
      this.status.textContent = "Sent ✓";
    } catch (err) {
      this.status.textContent = String(err).replace(/^Error:\s*/, "");
    }
  }

  private static line(who: string, cls = "") {
    const text = h("span");
    return { row: h("div", { class: `session-line ${cls}` }, h("i", { text: who }), text), text };
  }

  private static set(line: { row: HTMLElement; text: HTMLElement }, value: string | undefined) {
    line.row.style.display = value ? "" : "none";
    if (line.text.textContent !== (value ?? "")) line.text.textContent = value ?? "";
  }

  update(s: Session) {
    if (this.session?.id !== s.id) {
      this.status.textContent = "";
      this.input.value = "";
      this.input.style.display = "none";
    }
    this.session = s;
    // A message can't reach an open dialog: send them to the terminal instead.
    this.dialogHint.style.display = s.dialog ? "" : "none";
    this.replies.style.display = s.dialog || !s.pid ? "none" : "";
    const color = sessionAgent(s).color;
    this.dotEl.style.background = color;
    const title = s.title || s.name || s.project;
    if (this.titleEl.textContent !== title) this.titleEl.textContent = title;
    const flag = followUp(s, Date.now());
    const flagText = flag === "waiting" ? " · needs you" : flag ? ` · ${flag}` : "";
    const meta = `${s.project} · ${STATE_LABEL[s.state]}${flagText} · ${timeAgo(s.updatedAt)}`;
    if (this.metaEl.textContent !== meta) this.metaEl.textContent = meta;
    SessionCard.set(this.you, s.lastPrompt);
    SessionCard.set(this.claude, s.lastReply);
    const busy = s.state === "working" || s.state === "approval" || s.state === "question";
    // A pending question reads as what Claude asks you, not what it is doing.
    const asks = s.state === "question" || s.state === "approval";
    if (this.nowLabel.textContent !== (asks ? "Asks" : "Now")) this.nowLabel.textContent = asks ? "Asks" : "Now";
    SessionCard.set(this.now, busy ? s.step : undefined);
    const key = `${s.id}|${s.pid ?? ""}|${s.cwd}|${color}`;
    if (key !== this.buttonsKey) {
      this.buttonsKey = key;
      const fresh = sessionButtons(s, color);
      this.buttons.replaceWith(fresh);
      this.buttons = fresh;
    }
  }
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

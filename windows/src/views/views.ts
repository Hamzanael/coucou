// Island views — DOM ports of IslandViewContent.swift. Paddings, font sizes,
// colours and wording are copied from the Swift views so both platforms read
// identically.

import { h, svg, clear, dot } from "./dom";
import { ICONS } from "./icons";
import { Bridge } from "../core/bridge";
import { Ticker } from "./ticker";
import { State, type AgentTask } from "../core/state";
import { washRGBA, type IslandViewName, type Wash } from "../core/layout";
import { createMiniBot, pruneMiniBots } from "../mochi/minibots";
import { buildPrompt } from "./chat";
import { buildChoose, buildUpload, buildUploading } from "./upload";
import { buildCalendar } from "./calendar";
import { buildWorktrees } from "./worktrees";
import { buildHealth } from "./health";
import { buildPipelines } from "./pipelines";
import { buildDashboard } from "./dashboard";
import { renderTeleport } from "./teleport";
import { crew, isSessionAgent, liveSessions, SessionCard, selectedSession, stateLabel } from "./sessions";
import type { Session } from "../core/sessions";
import { SESSION_PREFIX } from "../core/crew";
import { answeredInput, askQuestions } from "../core/ask";
import { followUp } from "../core/followup";

const FLAG_LABEL = { waiting: "needs you", unfinished: "unfinished", stale: "stale" } as const;

/** focusSessionId value that shows the cloud-session chooser instead. */
const CLOUD = "__cloud__";
/** cardKey while the left card shows the (in-place) session card. */
const SESSION_CARD = "__session__";
import { renderIntegrationCard, type IntegrationCardHooks } from "./integrations";

export interface ViewActions {
  setView(v: IslandViewName): void;
  collapse(): void;
  /** Back to the clock pill (or compact without a clock) right away. */
  close(): void;
  /** Show this Claude Code session in the overview's left card. */
  focusSession(id: string): void;
  /** Expand mode on / off. */
  toggleDashboard(): void;
  /** Drop the island under the bar so a text field can take the keyboard. */
  requestKeyboard(): void;
  setFocus(id: string): void;
  openTerminal(): void;
  /** The ↗ button: opens whatever the focused pill points at. */
  openTarget(): void;
  openUrl(url: string): void;
  decide(d: "allow" | "deny"): void;
  /** Answer the pending AskUserQuestion with this tool input (answers added). */
  answerQuestion(updatedInput: Record<string, unknown>): void;
  /** Leave the pending question to Claude Code's dialog in the terminal. */
  answerInTerminal(): void;
  toggleSound(): void;
  setVolume(v: number): void;
  setAutoClose(seconds: number): void;
  openSettingsWindow(): void;
  blip(): void;
}

export interface ViewHost {
  el: HTMLElement;
  sync(): void;
  /** Called when the view becomes active, for views with a text field. */
  focus?(): void;
  /** Called each time the view comes on screen in the open island. */
  shown?(): void;
  /** Called every frame while the view is on screen. */
  tick?(nowMs: number): void;
}

// ── Shared pieces ─────────────────────────────────────────────────────────────

function card(wash: Wash, ...children: (Node | string)[]): HTMLElement {
  const el = h("div", { class: wash ? "card wash" : "card" }, ...children);
  if (wash) el.style.setProperty("--wash", washRGBA(wash));
  return el;
}

function btn(
  label: string,
  kind: "primary" | "secondary",
  onClick: () => void,
  kbd?: string,
): HTMLElement {
  return h(
    "button",
    { class: `btn ${kind}`, onclick: onClick },
    h("span", { text: label }),
    kbd ? h("span", { class: "kbd", text: kbd }) : null,
  );
}

/** AgentWho — coloured dot + task name + grey label. */
function agentWho(task: AgentTask | null, label: string): HTMLElement {
  const row = h("div", { class: "who-row" });
  if (task) {
    row.append(dot(task.color, 8), h("span", { class: "n", text: task.name }));
  }
  row.append(h("span", { text: label }));
  return row;
}

function stack(padLeft: number, padRight: number, ...children: Node[]): HTMLElement {
  const el = h("div", { class: "stack" }, ...children);
  el.style.padding = `4px ${padRight}px 4px ${padLeft}px`;
  return el;
}

// ── Header ────────────────────────────────────────────────────────────────────

export function buildHeader(actions: ViewActions): ViewHost {
  const tabHome = h("button", { class: "tab", title: "Overview", onclick: () => go("overview") }, svg(ICONS.house, 13));
  const tabChat = h("button", { class: "tab", title: "Ask", onclick: () => go("prompt") }, svg(ICONS.bubble, 13));
  const tabDrop = h("button", { class: "tab", title: "Drop", onclick: () => go("upload") }, svg(ICONS.plus, 13));
  const tabCalendar = h("button", { class: "tab", title: "Calendar", onclick: () => go("calendar") }, svg(ICONS.calendar, 13));
  const tabWorktrees = h("button", { class: "tab", title: "Worktrees", onclick: () => go("worktrees") }, svg(ICONS.branch, 13));
  const tabHealth = h("button", { class: "tab", title: "Health", onclick: () => go("health") }, svg(ICONS.pulse, 13));
  const tabPipelines = h("button", { class: "tab", title: "Pipelines", onclick: () => go("pipelines") }, svg(ICONS.pipeline, 13));

  const gearBtn = h("button", { title: "Settings", onclick: () => go("settings") }, svg(ICONS.gear, 14));
  const soundBtn = h("button", { title: "Mute", onclick: () => actions.toggleSound() }, svg(ICONS.speakerOn, 14));
  const closeBtn = h("button", { title: "Close", onclick: () => actions.close() }, svg(ICONS.xmark, 12));
  const expandBtn = h("button", { title: "Expand (F)", onclick: () => actions.toggleDashboard() }, svg(ICONS.expand, 13));

  function go(v: IslandViewName) {
    actions.blip();
    actions.setView(v);
  }

  const el = h(
    "div",
    { id: "header" },
    h("div", { class: "tabs" }, tabHome, tabChat, tabDrop, tabCalendar, tabWorktrees, tabHealth, tabPipelines),
    h("div", { class: "header-actions" }, expandBtn, gearBtn, soundBtn, closeBtn),
  );

  return {
    el,
    sync() {
      const v = State.view;
      tabHome.classList.toggle("on", v === "overview" || v === "empty");
      tabChat.classList.toggle("on", v === "prompt");
      tabDrop.classList.toggle("on", v === "upload");
      tabCalendar.classList.toggle("on", v === "calendar");
      tabCalendar.style.display = State.clock ? "" : "none";
      tabWorktrees.classList.toggle("on", v === "worktrees");
      tabWorktrees.style.display = State.clock ? "" : "none";
      tabHealth.classList.toggle("on", v === "health");
      tabHealth.style.display = State.clock ? "" : "none";
      tabPipelines.classList.toggle("on", v === "pipelines");
      tabPipelines.style.display = State.clock ? "" : "none";
      expandBtn.classList.toggle("on", v === "dashboard");
      expandBtn.style.display = State.clock ? "" : "none";
      gearBtn.classList.toggle("on", v === "settings");
      clear(gearBtn);
      gearBtn.append(svg(v === "settings" ? ICONS.gearFill : ICONS.gear, 14));
      clear(soundBtn);
      soundBtn.append(svg(State.settings.soundEnabled ? ICONS.speakerOn : ICONS.speakerOff, 14));
      el.style.opacity = v === "confused" ? "0" : "1";
    },
  };
}

// ── Overview ──────────────────────────────────────────────────────────────────

function buildOverview(actions: ViewActions): ViewHost {
  const ticker = new Ticker();
  const who = h("div", { class: "who" });
  const tickerBody = h("div", { class: "card-body" }, who, ticker.el);
  const leftBody = h("div", { class: "left-body" });
  const jump = h(
    "button",
    { class: "icon-btn jump", title: "Open", onclick: () => actions.openTarget() },
    svg(ICONS.arrowUpRight, 8),
  );
  const left = card(null, leftBody, jump);
  const pills = h("div", { class: "pills" });

  const right = card(null, pills);

  const el = h("div", { class: "view overview" },
    h("div", { class: "left" }, left),
    h("div", { class: "right" }, right),
  );

  const crewRows = new Map<string, { el: HTMLElement; sig: string }>();
  const sessionCard = new SessionCard(() => actions.requestKeyboard());
  const cloudPill = h(
    "div",
    { class: "pill cloud-pill", "data-nav": true, tabindex: "0", title: "Pull a cloud session", onclick: () => actions.focusSession(CLOUD) },
    h("span", { class: "lbl", text: "☁  Cloud" }),
  );
  let detailOpen = false;
  let lastFocus: string | null = null;
  let mode: "ticker" | "card" | null = null;
  let cardKey = "";

  const hooks: IntegrationCardHooks = {
    get detailOpen() {
      return detailOpen;
    },
    openDetail() {
      detailOpen = true;
      cardKey = "";
      State.notify();
    },
    closeDetail() {
      detailOpen = false;
      cardKey = "";
      State.notify();
    },
    openSettings: () => actions.openSettingsWindow(),
  };

  return {
    el,
    tick(nowMs: number) {
      if (mode === "ticker") ticker.tick(nowMs);
    },
    sync() {
      const task = State.focusTask;
      if (task?.id !== lastFocus) {
        lastFocus = task?.id ?? null;
        detailOpen = false;
        cardKey = "";
        mode = null;
      }

      // Claude with a live session keeps the ticker; every other
      // pill shows its own card, exactly like IntegrationCardView.
      const sessionActive =
        task?.id === "integration_claude" && (task.state !== "idle" || task.steps.length > 0);

      const sessions = liveSessions();
      const cloud = task?.id === "integration_claude" && State.focusSessionId === CLOUD;
      const shown = task?.id === "integration_claude" && !cloud ? selectedSession(sessions) : null;
      if (cloud) {
        if (mode !== "card" || cardKey !== CLOUD) {
          clear(leftBody);
          leftBody.append(h("div", { class: "session-card" }, renderTeleport()));
          mode = "card";
          cardKey = CLOUD;
        }
      } else if (shown) {
        if (mode !== "card" || cardKey !== SESSION_CARD) {
          clear(leftBody);
          leftBody.append(sessionCard.el);
          mode = "card";
          cardKey = SESSION_CARD;
        }
        sessionCard.update(shown);
      } else if (task && sessionActive) {
        if (mode !== "ticker") {
          clear(leftBody);
          leftBody.append(tickerBody);
          mode = "ticker";
          cardKey = "";
        }
        clear(who);
        who.append(
          dot(task.color, 7),
          h("span", { class: "name", text: task.name }),
          h("span", { class: "tool", text: task.source === "claudeCode" ? "Claude Code" : "n8n" }),
        );
        if (task.steps.length > 1) {
          who.append(h("span", {
            class: "count",
            text: `${Math.min(task.stepIndex + 1, task.steps.length)}/${task.steps.length}`,
          }));
        }
        ticker.sync(task);
      } else if (task) {
        const info = State.integrations[task.id];
        const key = [
          task.id, detailOpen, task.state, task.steps.join("|"),
          info?.loaded, info?.error, info?.configured,
          JSON.stringify(info?.data ?? {}),
        ].join("~");
        if (key !== cardKey) {
          cardKey = key;
          mode = "card";
          clear(leftBody);
          leftBody.append(renderIntegrationCard(task, hooks));
        }
      }

      jump.style.display = detailOpen ? "none" : "";

      // The crew: every Claude Code session as an avatar, then the integrations.
      // Rows are updated in place, never rebuilt: sessions change state every few
      // seconds, and a rebuild under the pointer swallowed clicks and reset the
      // scroll position.
      const members = crew(sessions);
      const now = Date.now();
      const bySession = new Map(sessions.map((s) => [`${SESSION_PREFIX}${s.id}`, s]));
      const selectedId = cloud ? CLOUD : shown ? `${SESSION_PREFIX}${shown.id}` : "";
      const wanted = new Set(members.map((t) => t.id));
      for (const [id, row] of crewRows) {
        if (!wanted.has(id)) {
          row.el.remove();
          crewRows.delete(id);
        }
      }
      for (const t of members) {
        // Only a colour change (the avatar's body) rebuilds a row; badges update in place.
        const sig = t.color;
        let row = crewRows.get(t.id);
        if (!row || row.sig !== sig) {
          const el = buildPill(t, actions);
          row?.el.replaceWith(el);
          row = { el, sig };
          crewRows.set(t.id, row);
        }
        const lbl = row.el.querySelector(".lbl");
        if (lbl && lbl.textContent !== t.name) lbl.textContent = t.name;
        const state = row.el.querySelector(".pill-state");
        const session = bySession.get(t.id);
        const flag = session ? followUp(session, now) : null;
        const stateText = flag ? FLAG_LABEL[flag] : isSessionAgent(t) ? stateLabel(t.state as Session["state"]) : "";
        if (state && state.textContent !== stateText) state.textContent = stateText;
        if (state) state.className = flag ? `pill-state flag-${flag}` : "pill-state";
        row.el.classList.toggle("selected", t.id === selectedId);
        setPillBadge(row.el, t.pillBadge ?? null);
      }
      const order = [...members.map((t) => crewRows.get(t.id)!.el), cloudPill];
      order.forEach((el, i) => {
        if (pills.children[i] !== el) pills.insertBefore(el, pills.children[i] ?? null);
      });
      cloudPill.classList.toggle("selected", selectedId === CLOUD);
      pruneMiniBots();
    },
  };
}

function buildPill(task: AgentTask, actions: ViewActions): HTMLElement {
  const label = task.name;
  const canvas = createMiniBot(task, 24);
  const pill = h(
    "div",
    {
      class: "pill",
      "data-nav": true,
      tabindex: "0",
      title: isSessionAgent(task) ? task.steps[task.steps.length - 1] ?? task.name : task.name,
      onclick: () => {
        // A session avatar fills the left card; the System pill opens Health.
        if (isSessionAgent(task)) actions.focusSession(task.id.slice(SESSION_PREFIX.length));
        else if (task.id === "integration_system") actions.setView("health");
        else actions.setFocus(task.id);
      },
    },
    canvas,
    h("span", { class: "lbl", text: label }),
    isSessionAgent(task) ? h("span", { class: "pill-state", text: stateLabel(task.state as Session["state"]) }) : null,
  );
  pill.style.borderColor = `${task.color}24`;
  pill.addEventListener("mouseenter", () => {
    diagHover(task.name);
    pill.style.background = `${task.color}2e`;
    pill.style.borderColor = `${task.color}8c`;
    pill.style.boxShadow = `0 2px 10px ${task.color}59`;
    (pill.querySelector(".lbl") as HTMLElement).style.color = lighten(task.color, 0.3);
  });
  pill.addEventListener("mouseleave", () => {
    pill.style.background = "";
    pill.style.borderColor = `${task.color}24`;
    pill.style.boxShadow = "";
    (pill.querySelector(".lbl") as HTMLElement).style.color = "";
  });

  setPillBadge(pill, task.pillBadge ?? null);
  pill.addEventListener("mousedown", () => diagClick("press", task.name));
  pill.addEventListener("click", () => {
    diagClick("click", task.name);
    // Visible proof of the click, even when nothing else on screen changes.
    pill.classList.remove("flash");
    void pill.offsetWidth;
    pill.classList.add("flash");
  });
  return pill;
}

/** Puts (or clears) a pill's badge in place, so the row itself is never rebuilt. */
function setPillBadge(pill: HTMLElement, badge: AgentTask["pillBadge"]) {
  const current = pill.querySelector<HTMLElement>(".pill-badge");
  if ((current?.dataset.kind ?? null) === (badge ?? null)) return;
  current?.remove();
  if (!badge) return;
  const colors = { approval: "#F5A524", finished: "#22C55E", error: "#F4505E" } as const;
  const icons = { approval: ICONS.bang, finished: ICONS.check, error: ICONS.xmark } as const;
  const inner = h("i", { style: `background:${colors[badge]}` }, svg(icons[badge], 6, { stroke: badge === "finished" ? 3 : 0 }));
  const el = h("div", { class: "pill-badge" }, inner);
  el.dataset.kind = badge;
  el.style.boxShadow = `0 0 4px ${colors[badge]}99`;
  pill.append(el);
}

let lastClickLog = 0;
/** Temporary: proves whether a row receives presses and clicks. */
function diagClick(what: string, name: string) {
  const now = Date.now();
  if (what === "press" && now - lastClickLog < 300) return;
  lastClickLog = now;
  void Bridge.log(`diag row ${what} ${name}`);
}

let lastHoverLog = 0;
/** Temporary: proves whether rows receive the pointer (one line per 3 s). */
function diagHover(name: string) {
  const now = Date.now();
  if (now - lastHoverLog < 3000) return;
  lastHoverLog = now;
  void Bridge.log(`diag hover row ${name}`);
}

function lighten(hex: string, amount: number): string {
  const v = parseInt(hex.replace("#", ""), 16);
  const c = [(v >> 16) & 255, (v >> 8) & 255, v & 255].map((x) =>
    Math.min(255, Math.round(x + amount * 255)),
  );
  return `rgb(${c[0]},${c[1]},${c[2]})`;
}

// ── Empty ─────────────────────────────────────────────────────────────────────

function buildEmpty(actions: ViewActions): ViewHost {
  const body = h(
    "div",
    { class: "stack", style: "padding:0 18px 0 118px;flex-direction:row;align-items:center;gap:16px" },
    h(
      "div",
      { style: "display:flex;flex-direction:column;gap:5px" },
      h("div", { class: "title", text: "Nothing running right now." }),
      h("div", { class: "sub", text: "Drop a file or window, or ask me anything." }),
    ),
    h("div", { class: "grow" }),
    btn("Ask Claude", "primary", () => actions.setView("prompt")),
  );
  return { el: h("div", { class: "view" }, card(null, body)), sync() {} };
}

// ── Approval ──────────────────────────────────────────────────────────────────

/** AskUserQuestion: the questions with their options; answers go back to Claude Code. */
function buildQuestionCard(actions: ViewActions, input: Record<string, unknown>): HTMLElement {
  const questions = askQuestions(input);
  const picks: Record<string, string[]> = {};
  const send = btn("Send answer", "primary", () => {
    const answered = answeredInput(input, picks);
    if (answered) actions.answerQuestion(answered);
  });
  const refresh = () => send.classList.toggle("disabled", !answeredInput(input, picks));
  const body = h("div", { class: "ask" });
  for (const q of questions) {
    const chips = h("div", { class: "ask-options" });
    for (const o of q.options) {
      const chip = h("button", { class: "reply-chip ask-chip", "data-nav": true, text: o.label, title: o.description ?? "" });
      chip.addEventListener("click", () => {
        const now = picks[q.question] ?? [];
        picks[q.question] = q.multiSelect
          ? now.includes(o.label) ? now.filter((l) => l !== o.label) : [...now, o.label]
          : [o.label];
        for (const c of Array.from(chips.children)) c.classList.toggle("on", picks[q.question].includes((c as HTMLElement).textContent ?? ""));
        refresh();
      });
      chips.append(chip);
    }
    body.append(h("div", { class: "ask-q", text: q.question }), chips);
  }
  body.append(h("div", { class: "actions" }, btn("Answer in terminal", "secondary", () => actions.answerInTerminal()), send));
  refresh();
  return body;
}

function buildApproval(actions: ViewActions): ViewHost {
  const who = h("div");
  const code = h("div", { class: "code" });
  const row = h("div", { class: "actions" });
  const el = h("div", { class: "view" }, card("amber", stack(116, 16, who, code, row)));
  let rowKey = "";
  return {
    el,
    sync() {
      clear(who);
      who.append(agentWho(State.focusTask, State.pendingApproval?.tool === "AskUserQuestion" ? "asks you" : "needs permission"));
      // The whole point of approving here rather than in the terminal: this line
      // is the command, the file path or the URL being authorised, not just the
      // name of the tool asking.
      code.textContent = State.pendingApproval?.command || State.pendingApproval?.tool || "…";
      // Two buttons, built once. Rebuilding them between a mouse-down and a
      // mouse-up would swallow the click, and there is nothing left to vary:
      // "Always" is gone until the remembered-rules list exists to back it.
      // A question gets its own card, built once per request.
      const req = State.pendingApproval;
      const key = req?.tool === "AskUserQuestion" ? `ask:${req.requestId}` : "built";
      code.style.display = key === "built" ? "" : "none";
      row.style.display = key === "built" ? "" : "block";
      if (rowKey === key) return;
      rowKey = key;
      clear(row);
      if (req && key !== "built") {
        row.append(buildQuestionCard(actions, req.input));
        return;
      }
      row.append(
        btn("Deny", "secondary", () => actions.decide("deny"), "N"),
        btn("Allow", "primary", () => actions.decide("allow"), "Y"),
      );
    },
  };
}

// ── Question ──────────────────────────────────────────────────────────────────

function buildQuestion(): ViewHost {
  const who = h("div");
  const title = h("div", { class: "title" });
  const row = h("div", { class: "actions" });
  const el = h("div", { class: "view" }, card("cyan", stack(116, 16, who, title, row)));
  return {
    el,
    sync() {
      clear(who);
      who.append(agentWho(State.focusTask, "Claude Code is asking a question"));
      const task = State.focusTask;
      title.textContent = task?.steps.at(-1) ?? "Claude needs an answer.";
      clear(row);
      row.append(h("div", { class: "sub", text: "Answer in your terminal — Coucou can't reply for you yet." }));
    },
  };
}

// ── Error ─────────────────────────────────────────────────────────────────────

function buildError(actions: ViewActions): ViewHost {
  const who = h("div");
  const title = h("div", { class: "title", text: "Workflow stopped." });
  const detail = h("div", { class: "detail" });
  const row = h("div", { class: "actions" },
    btn("Retry", "primary", () => actions.setView(State.defaultView())),
    btn("Open in n8n", "secondary", () => actions.openUrl("")),
  );
  const el = h("div", { class: "view" }, card("red", stack(116, 16, who, title, detail, row)));
  return {
    el,
    sync() {
      const task = State.focusTask;
      clear(who);
      who.append(agentWho(task, task?.source === "n8n" ? "n8n" : "Claude Code"));
      title.textContent = task?.source === "n8n" ? "Workflow stopped." : "Session stopped on an error.";
      detail.textContent = task?.steps.at(-1) ?? "No detail available.";
    },
  };
}

// ── Finished ──────────────────────────────────────────────────────────────────

function buildFinished(actions: ViewActions): ViewHost {
  const who = h("div");
  const title = h("div", { class: "title" });
  const row = h("div", { class: "actions" },
    btn("Open terminal", "primary", () => actions.openTerminal()),
    btn("OK", "secondary", () => actions.collapse()),
  );
  const el = h("div", { class: "view" }, card("green", stack(116, 16, who, title, row)));
  return {
    el,
    sync() {
      clear(who);
      who.append(agentWho(State.focusTask, "Claude Code finished"));
      title.textContent = State.focusTask?.steps.at(-1) ?? "Session finished";
    },
  };
}

// ── Confused ──────────────────────────────────────────────────────────────────

function buildConfused(): ViewHost {
  const body = h(
    "div",
    { class: "stack", style: "padding:0 18px 0 128px" },
    h("div", { class: "title", text: "Too many hits at once." }),
    h("div", { class: "sub", text: "Give me a sec — back to work in three seconds." }),
  );
  return { el: h("div", { class: "view" }, card("pink", body)), sync() {} };
}

// ── Note ──────────────────────────────────────────────────────────────────────

function buildNote(): ViewHost {
  const title = h("div", { class: "title" });
  const el = h("div", { class: "view" }, card(null, h("div", { class: "stack", style: "padding:0 18px 0 98px" }, title)));
  return {
    el,
    sync() {
      title.textContent = State.noteMessage ?? "";
    },
  };
}

// ── In-island settings ────────────────────────────────────────────────────────

function buildSettings(actions: ViewActions): ViewHost {
  const soundSwitch = h("button", { class: "switch", onclick: () => actions.toggleSound() });
  const volume = h("input", {
    type: "range", min: "0", max: "0.2", step: "0.005",
    oninput: (e: Event) => actions.setVolume(Number((e.target as HTMLInputElement).value)),
  }) as HTMLInputElement;
  const autoLabel = h("span", {});
  const segButtons = [10, 15, 30].map((s) =>
    h("button", { onclick: () => actions.setAutoClose(s) }, `${s}s`),
  );
  const claudeBadge = h("span", { class: "status-badge" });
  const apiBadge = h("span", { class: "status-badge" });

  const rows = h(
    "div",
    { class: "settings-rows" },
    h("div", { class: "settings-row" }, soundSwitch, h("span", { text: "Sound" }), volume),
    h(
      "div",
      { class: "settings-row" },
      svg(ICONS.timer, 12),
      autoLabel,
      h("div", { class: "seg" }, ...segButtons),
    ),
    h(
      "div",
      { class: "settings-row", style: "gap:14px" },
      claudeBadge,
      apiBadge,
      h("div", { class: "grow" }),
      h("button", {
        class: "link-btn",
        style: "color:#8e939c;font-size:11.5px",
        text: "Settings…",
        onclick: () => actions.openSettingsWindow(),
      }),
    ),
  );

  const el = h("div", { class: "view" },
    card(null, h("div", { class: "stack", style: "padding:14px 16px 14px 84px" }, rows)));

  return {
    el,
    sync() {
      const s = State.settings;
      soundSwitch.classList.toggle("on", s.soundEnabled);
      volume.value = String(s.soundVolume);
      volume.style.opacity = s.soundEnabled ? "1" : "0.4";
      autoLabel.textContent = `Auto-close · ${Math.round(s.autoCloseInterval)}s`;
      segButtons.forEach((b, i) => b.classList.toggle("on", s.autoCloseInterval === [10, 15, 30][i]));
      clear(claudeBadge);
      claudeBadge.append(
        dot(s.hooksInstalled ? "#22C55E" : "#F4505E", 6),
        h("span", { text: "Claude Code" }),
      );
      clear(apiBadge);
      apiBadge.append(dot("#F4505E", 6), h("span", { text: "API" }));
    },
  };
}

// ── Placeholders filled in later stages ───────────────────────────────────────

function buildPlaceholder(title: string, sub: string): ViewHost {
  const body = h(
    "div",
    { class: "stack", style: "padding:0 18px 0 118px" },
    h("div", { class: "title", text: title }),
    h("div", { class: "sub", text: sub }),
  );
  return { el: h("div", { class: "view" }, card(null, body)), sync() {} };
}

// ── Registry ──────────────────────────────────────────────────────────────────

export function buildViews(
  actions: ViewActions,
  onChatHeightChange: () => void,
): Map<IslandViewName, ViewHost> {
  const map = new Map<IslandViewName, ViewHost>();
  map.set("overview", buildOverview(actions));
  map.set("empty", buildEmpty(actions));
  map.set("approval", buildApproval(actions));
  map.set("question", buildQuestion());
  map.set("error", buildError(actions));
  map.set("finished", buildFinished(actions));
  map.set("confused", buildConfused());
  map.set("note", buildNote());
  map.set("settings", buildSettings(actions));
  map.set("prompt", buildPrompt(onChatHeightChange));
  map.set("upload", buildUpload());
  map.set("uploading", buildUploading());
  map.set("choose", buildChoose(actions));
  map.set("calendar", buildCalendar(() => actions.blip()));
  map.set("worktrees", buildWorktrees(() => actions.blip()));
  map.set("health", buildHealth(() => actions.blip()));
  map.set("pipelines", buildPipelines());
  map.set("dashboard", buildDashboard(() => actions.blip()));
  // Not in the Windows v1: sending a file by email, window attach + web result.
  map.set("mail", buildPlaceholder("Sending by email isn't in this version.", ""));
  map.set("searching", buildPlaceholder("Claude is searching…", ""));
  map.set("result", buildPlaceholder("Result", ""));
  return map;
}

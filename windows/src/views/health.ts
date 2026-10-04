// Health view: one row per check, coloured by level, then the free-up actions.
// Every action needs two clicks (Free → Confirm); root-only ones are a command
// to copy, never run here.

import { Bridge, type CleanupItem } from "../core/bridge";
import { State } from "../core/state";
import { h, clear, dot } from "./dom";
import type { ViewHost } from "./views";

const COLOR = { ok: "#22C55E", warn: "#F5A524", bad: "#F4505E" } as const;

const gb = (bytes: number | null) =>
  bytes == null ? "" : bytes >= 1e9 ? `${(bytes / 1e9).toFixed(1)} GB` : `${Math.round(bytes / 1e6)} MB`;

export function buildHealth(blip: () => void): ViewHost {
  const checks = h("div", { class: "health-checks" });
  const status = h("span", { class: "wt-status" });
  const actions = h("div", { class: "health-actions" });
  const el = h(
    "div",
    { class: "view" },
    h(
      "div",
      { class: "card" },
      h(
        "div",
        { class: "health" },
        checks,
        h("div", { class: "wt-head" }, h("b", { text: "Free up space" }), status),
        actions,
      ),
    ),
  );

  let rendered: unknown = undefined;
  let items: CleanupItem[] = [];
  let armed: string | null = null;
  let busy = false;

  function renderChecks() {
    const report = State.health;
    if (report === rendered) return;
    rendered = report;
    clear(checks);
    if (!report) {
      checks.append(h("div", { class: "wt-empty", text: "Checking… (first sample within a minute)" }));
      return;
    }
    for (const c of report.checks) {
      checks.append(
        h("div", { class: "health-row" }, dot(COLOR[c.level], 8), h("b", { text: c.label }), h("span", { text: c.value })),
      );
    }
  }

  function renderActions() {
    clear(actions);
    for (const item of items) {
      const button = item.command
        ? h("button", {
            class: "btn secondary",
            "data-nav": true,
            text: "Copy",
            title: item.command,
            onclick: () => {
              blip();
              void navigator.clipboard.writeText(item.command ?? "");
              status.textContent = "Command copied — run it in a terminal";
            },
          })
        : h("button", {
            class: armed === item.id ? "btn primary danger" : "btn secondary",
            "data-nav": true,
            disabled: busy || !item.available,
            text: armed === item.id ? `Confirm ${gb(item.sizeBytes)}` : "Free",
            onclick: () => void act(item),
          });
      const note = item.detail ? `${item.note} · ${item.detail}` : item.note;
      actions.append(
        h(
          "div",
          { class: "health-action", title: note },
          h("b", { text: item.label }),
          h("span", { class: "health-size", text: gb(item.sizeBytes) }),
          h("span", { class: "health-note", text: note }),
          button,
        ),
      );
    }
  }

  async function scan() {
    status.textContent = "Measuring…";
    items = (await Bridge.cleanupScan()) ?? [];
    status.textContent = "";
    renderActions();
  }

  async function act(item: CleanupItem) {
    blip();
    if (armed !== item.id) {
      armed = item.id;
      status.textContent = `Click Confirm to free ${item.label.toLowerCase()}`;
      renderActions();
      return;
    }
    armed = null;
    busy = true;
    status.textContent = `Freeing ${item.label.toLowerCase()}…`;
    renderActions();
    try {
      status.textContent = await Bridge.cleanupRun(item.id);
    } catch (err) {
      status.textContent = String(err).replace(/^Error:\s*/, "");
    }
    busy = false;
    await scan();
  }

  return {
    el,
    sync: renderChecks,
    shown() {
      armed = null;
      void scan();
    },
  };
}

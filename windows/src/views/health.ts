// Health view: one row per check, coloured by level.

import { State } from "../core/state";
import { h, clear, dot } from "./dom";
import type { ViewHost } from "./views";

const COLOR = { ok: "#22C55E", warn: "#F5A524", bad: "#F4505E" } as const;

export function buildHealth(): ViewHost {
  const list = h("div", { class: "health" });
  const el = h("div", { class: "view" }, h("div", { class: "card" }, list));
  let rendered: unknown = undefined;
  return {
    el,
    sync() {
      const report = State.health;
      if (report === rendered) return;
      rendered = report;
      clear(list);
      if (!report) {
        list.append(h("div", { class: "wt-empty", text: "Checking… (first sample within a minute)" }));
        return;
      }
      for (const c of report.checks) {
        list.append(
          h("div", { class: "health-row" }, dot(COLOR[c.level], 8), h("b", { text: c.label }), h("span", { text: c.value })),
        );
      }
    },
  };
}

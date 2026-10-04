// Stale worktrees: scan, pick, remove. Nothing is selected until the user ticks it.

import { Bridge, type StaleWorktree } from "../core/bridge";
import { h, clear } from "./dom";
import type { ViewHost } from "./views";

const REASON: Record<StaleWorktree["reason"], string> = {
  missing: "folder missing",
  merged: "branch merged",
  upstreamGone: "upstream gone",
  idle: "idle",
};

const base = (p: string) => p.replace(/\/+$/, "").split("/").pop() ?? p;
const mb = (kb: number | null) => (kb == null ? "" : `${(kb / 1024).toFixed(kb > 10240 ? 0 : 1)} MB`);

export function buildWorktrees(blip: () => void): ViewHost {
  const list = h("div", { class: "wt-list" });
  const status = h("span", { class: "wt-status" });
  const clean = h("button", { class: "btn primary" }) as HTMLButtonElement;
  const rescan = h("button", { class: "btn secondary", text: "Rescan" });
  const el = h(
    "div",
    { class: "view" },
    h(
      "div",
      { class: "card" },
      h(
        "div",
        { class: "wt" },
        h("div", { class: "wt-head" }, h("b", { text: "Stale worktrees" }), status),
        list,
        h("div", { class: "wt-actions" }, rescan, clean),
      ),
    ),
  );
  let found: StaleWorktree[] = [];
  const selected = new Set<string>();

  function render() {
    clear(list);
    if (found.length === 0) list.append(h("div", { class: "wt-empty", text: "Nothing stale" }));
    for (const w of found) {
      const box = h("input", { type: "checkbox" }) as HTMLInputElement;
      box.checked = selected.has(w.path);
      box.onchange = () => {
        if (box.checked) selected.add(w.path);
        else selected.delete(w.path);
        render();
      };
      const why = w.reason === "idle" && w.idleDays != null ? `idle ${w.idleDays}d` : REASON[w.reason];
      list.append(
        h(
          "label",
          { class: "wt-row", title: w.path, "data-nav": true, tabindex: "0" },
          box,
          h("b", { text: base(w.repo) }),
          h("span", { text: w.branch ?? base(w.path) }),
          h("span", { class: "wt-why", text: why }),
          h("span", { class: "wt-size", text: mb(w.sizeKb) }),
        ),
      );
    }
    clean.disabled = selected.size === 0;
    clean.textContent = selected.size ? `Clean ${selected.size}` : "Clean selected";
  }

  async function scan() {
    status.textContent = "Scanning…";
    found = (await Bridge.worktreesScan()) ?? [];
    for (const p of [...selected]) if (!found.some((w) => w.path === p)) selected.delete(p);
    status.textContent = `${found.length} found`;
    render();
  }

  rescan.addEventListener("click", () => {
    blip();
    void scan();
  });
  clean.addEventListener("click", async () => {
    if (selected.size === 0) return;
    blip();
    const results = (await Bridge.worktreesRemove([...selected])) ?? [];
    const failed = results.filter((r) => !r.ok);
    selected.clear();
    await scan();
    // Shown after the rescan so its "N found" doesn't overwrite the outcome.
    status.textContent = failed.length
      ? `${results.length - failed.length} removed · ${failed.length} refused: ${failed[0].message}`
      : `${results.length} removed`;
  });

  render();
  return {
    el,
    sync() {},
    shown() {
      void scan();
    },
  };
}

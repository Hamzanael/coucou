// Pipelines view: the latest GitHub Actions run per workflow, on main and on
// each open PR, grouped by repo. Enter / click opens the run on GitHub.

import { Bridge, type PipelineRun } from "../core/bridge";
import { State } from "../core/state";
import { timeAgo } from "./integrations";
import { h, clear } from "./dom";
import type { ViewHost } from "./views";

function badge(run: PipelineRun): { text: string; color: string } {
  if (run.status !== "completed") return { text: run.status === "queued" ? "queued" : "running", color: "#3B9EFF" };
  switch (run.conclusion) {
    case "success":
      return { text: "passed", color: "#22C55E" };
    case "failure":
      return { text: "failed", color: "#F4505E" };
    case "cancelled":
      return { text: "cancelled", color: "#8E939C" };
    default:
      return { text: run.conclusion ?? "done", color: "#F5A524" };
  }
}

const repoName = (repo: string) => repo.split("/").pop() ?? repo;

export function buildPipelines(): ViewHost {
  const list = h("div", { class: "pipes" });
  const el = h("div", { class: "view" }, h("div", { class: "card" }, list));
  let rendered: unknown = undefined;

  return {
    el,
    sync() {
      const update = State.pipelines;
      if (update === rendered) return;
      rendered = update;
      clear(list);
      if (State.settings.pipelineRepos.length === 0) {
        list.append(h("div", { class: "wt-empty", text: "Add repos (owner/name) under Settings… → Pipelines" }));
        return;
      }
      if (!update) {
        list.append(h("div", { class: "wt-empty", text: "Loading… (first check within a minute)" }));
        return;
      }
      if (update.error) list.append(h("div", { class: "pipe-error", text: update.error }));
      let repo = "";
      for (const run of update.rows) {
        if (run.repo !== repo) {
          repo = run.repo;
          list.append(h("div", { class: "pipe-repo", text: repoName(repo) }));
        }
        const b = badge(run);
        list.append(
          h(
            "button",
            {
              class: "pipe-row",
              "data-nav": true,
              title: run.prTitle ? `${run.prTitle} — ${run.url}` : run.url,
              onclick: () => void Bridge.openUrl(run.url),
            },
            h("span", { class: "pipe-state", style: `color:${b.color};border-color:${b.color}55`, text: b.text }),
            h("b", { text: run.workflow }),
            h("span", { class: "pipe-branch", text: run.prTitle ? `${run.branch} · ${run.prTitle}` : run.branch }),
            h("span", { class: "pipe-age", text: timeAgo(run.createdAt) }),
          ),
        );
      }
    },
  };
}

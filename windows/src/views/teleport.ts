// "Pull a cloud session": one button per repo; Ghostty opens there running
// `claude --teleport`, straight into the session when a claude.ai/code link is
// on the clipboard, otherwise into Claude's own picker of your cloud sessions.

import { Bridge } from "../core/bridge";
import { State } from "../core/state";
import { h } from "./dom";

const base = (p: string) => p.replace(/\/+$/, "").split("/").pop() ?? p;

/** Folders worth teleporting into: the running sessions' repos, then the Pipelines repos. */
export function teleportDirs(): string[] {
  const dirs = State.sessions.list(Date.now()).map((s) => s.cwd).filter(Boolean);
  if (State.home) {
    for (const repo of State.settings.pipelineRepos) dirs.push(`${State.home}/IdeaProjects/${base(repo)}`);
  }
  return [...new Set(dirs)];
}

async function clipboardText(): Promise<string | null> {
  try {
    return await navigator.clipboard.readText();
  } catch {
    return null;
  }
}

export function renderTeleport(): HTMLElement {
  const status = h("div", { class: "tp-status", text: "Copy a claude.ai/code link first to jump straight to it." });
  const buttons = h("div", { class: "tp-dirs" });
  for (const dir of teleportDirs()) {
    buttons.append(
      h("button", {
        class: "btn secondary",
        "data-nav": true,
        title: dir,
        text: base(dir),
        onclick: async () => {
          try {
            status.textContent = await Bridge.teleport(dir, await clipboardText());
          } catch (err) {
            status.textContent = String(err).replace(/^Error:\s*/, "");
          }
        },
      }),
    );
  }
  return h(
    "div",
    { class: "tp" },
    h("b", { text: "☁  Pull a cloud session into…" }),
    buttons,
    status,
  );
}

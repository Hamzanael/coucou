// The overview's crew: every Claude Code session as its own mini Mochi, next to
// the integration pills, plus the keyboard's tab order.

import { colorForProject } from "./layout";
import type { Session } from "./sessions";
import type { AgentTask } from "./state";

export const SESSION_PREFIX = "session:";

/** A session as an avatar: coloured per project, animated by its state. */
export function sessionAgent(s: Session): AgentTask {
  return {
    id: `${SESSION_PREFIX}${s.id}`,
    name: s.project,
    color: colorForProject(s.project),
    state: s.state,
    stepIndex: Math.max(0, s.steps.length - 1),
    steps: s.steps,
    source: "claudeCode",
    isIntegration: false,
    sessionCwd: s.cwd,
  };
}

/** The tab `dir` steps away from `current`, wrapping; the first one when off-tab. */
export function nextTab<T extends string>(tabs: T[], current: string, dir: 1 | -1): T {
  const i = tabs.indexOf(current as T);
  if (i < 0) return tabs[0];
  return tabs[(i + dir + tabs.length) % tabs.length];
}

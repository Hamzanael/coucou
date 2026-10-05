// Which Claude Code sessions need following up, and which Coucou may nudge on
// its own. Pure: the island decides what to show and send from these.

import type { Session } from "./sessions";

/** Silence after which an open session counts as stale. */
export const STALE_MS = 30 * 60_000;

export type FollowUp = "waiting" | "unfinished" | "stale";

function open(s: Session): "waiting" | "unfinished" | null {
  if (s.state === "approval" || s.state === "question") return "waiting";
  // A turn that ended on a question is Claude waiting for an answer too.
  if ((s.state === "finished" || s.state === "idle") && s.lastReply?.trim().endsWith("?")) return "waiting";
  if (s.state === "error" || s.interrupted) return "unfinished";
  return null;
}

export function followUp(s: Session, now: number): FollowUp | null {
  const reason = open(s);
  if (!reason) return null;
  return now - s.updatedAt >= STALE_MS ? "stale" : reason;
}

/**
 * Sessions Coucou may tell to carry on by itself: stale and unfinished, once
 * each. A session waiting on a question or an approval is never answered for
 * you — that one is only flagged.
 */
export function nudgeCandidates(sessions: Session[], nudged: Set<string>, now: number): Session[] {
  return sessions.filter(
    (s) => !!s.pid && !nudged.has(s.id) && followUp(s, now) === "stale" && open(s) === "unfinished",
  );
}

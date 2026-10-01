// The time shown on the island where it stands in for the desktop's top-bar
// clock. Updated on the minute by a timer of its own, so the parked frame loop
// never has to wake up for it.

import { State } from "../core/state";
import { h } from "./dom";

export function formatTime(date: Date): string {
  return date.toLocaleTimeString(undefined, {
    hour: "numeric",
    minute: "2-digit",
    hour12: State.clock?.hour12 ?? false,
  });
}

/** Same pieces as GNOME's clock: optional weekday, optional date, then the time. */
function clockText(now: Date, short: boolean): string {
  const clock = State.clock;
  const parts: string[] = [];
  if (clock?.showWeekday || short) parts.push(now.toLocaleDateString(undefined, { weekday: "short" }));
  if (clock?.showDate && !short) parts.push(now.toLocaleDateString(undefined, { month: "short", day: "numeric" }));
  parts.push(formatTime(now));
  return parts.join("  ");
}

export class ClockFace {
  readonly el = h("div", { id: "clock" });
  private short = false;
  private timer: number | null = null;

  start() {
    if (this.timer != null) return;
    const tick = () => {
      this.render();
      this.timer = window.setTimeout(tick, 60_000 - (Date.now() % 60_000) + 50);
    };
    tick();
  }

  /** The compact island shares the bar with Mochi and the minis: time only. */
  setShort(short: boolean) {
    if (short === this.short) return;
    this.short = short;
    this.render();
  }

  private render() {
    this.el.textContent = clockText(new Date(), this.short);
  }
}

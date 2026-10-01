// Calendar view — what clicking the top-bar clock opens: a month grid and the
// selected day's events, read from the desktop's own calendars.

import { Bridge, type CalendarEvent } from "../core/bridge";
import { formatTime } from "./clock";
import { clear, h, svg } from "./dom";
import { ICONS } from "./icons";
import type { ViewHost } from "./views";

const DAY_MS = 86_400_000;
const GRID_DAYS = 42;
/** Events are re-read when the view comes back after this long. */
const STALE_MS = 60_000;

function startOfDay(d: Date): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate());
}

function addDays(d: Date, n: number): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate() + n);
}

function sameDay(a: Date, b: Date): boolean {
  return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
}

/** 0 = Sunday … 6 = Saturday, from the locale (Intl `weekInfo` uses 1–7, Monday first). */
function firstWeekday(): number {
  const locale = new Intl.Locale(navigator.language) as Intl.Locale & {
    getWeekInfo?: () => { firstDay: number };
    weekInfo?: { firstDay: number };
  };
  const firstDay = locale.getWeekInfo?.().firstDay ?? locale.weekInfo?.firstDay ?? 7;
  return firstDay % 7;
}

/** Six full weeks around the month — the same range GNOME's own clock asks for. */
function gridStart(month: Date): Date {
  const first = new Date(month.getFullYear(), month.getMonth(), 1);
  return addDays(first, -((first.getDay() - firstWeekday() + 7) % 7));
}

function eventsOn(events: CalendarEvent[], day: Date): CalendarEvent[] {
  const from = day.getTime() / 1000;
  const to = addDays(day, 1).getTime() / 1000;
  return events.filter((e) => e.start < to && Math.max(e.end, e.start + 1) > from);
}

/** All-day events run midnight to midnight; GNOME sends them as such. */
function isAllDay(e: CalendarEvent): boolean {
  const start = new Date(e.start * 1000);
  const end = new Date(e.end * 1000);
  return e.end > e.start &&
    start.getTime() === startOfDay(start).getTime() && end.getTime() === startOfDay(end).getTime();
}

function eventTime(e: CalendarEvent): string {
  if (isAllDay(e)) return "All day";
  const start = formatTime(new Date(e.start * 1000));
  return e.end > e.start ? `${start} – ${formatTime(new Date(e.end * 1000))}` : start;
}

export function buildCalendar(blip: () => void): ViewHost {
  let month = startOfDay(new Date());
  let selected = startOfDay(new Date());
  let events: CalendarEvent[] = [];
  let loadedKey = "";
  let loadedAt = 0;
  let request = 0;

  const title = h("button", { class: "cal-title", title: "Today", onclick: () => goToday() });
  const prev = h("button", { class: "cal-nav", title: "Previous month", onclick: () => shiftMonth(-1) },
    svg(ICONS.chevronLeft, 11, { stroke: 2.4 }));
  const next = h("button", { class: "cal-nav", title: "Next month", onclick: () => shiftMonth(1) },
    svg(ICONS.chevronRight, 11, { stroke: 2.4 }));
  const weekdays = h("div", { class: "cal-weekdays" });
  const grid = h("div", { class: "cal-grid" });
  const dayTitle = h("div", { class: "cal-day-title" });
  const list = h("div", { class: "cal-events" });

  const body = h(
    "div",
    { class: "cal" },
    h("div", { class: "cal-month" }, h("div", { class: "cal-head" }, prev, title, next), weekdays, grid),
    h("div", { class: "cal-day" }, dayTitle, list),
  );
  const el = h("div", { class: "view" }, h("div", { class: "card" }, body));

  function goToday() {
    blip();
    month = startOfDay(new Date());
    selected = startOfDay(new Date());
    render();
  }

  function shiftMonth(delta: number) {
    blip();
    month = new Date(month.getFullYear(), month.getMonth() + delta, 1);
    render();
  }

  function load(start: Date) {
    const since = start.getTime() / 1000;
    const until = addDays(start, GRID_DAYS).getTime() / 1000;
    const key = `${since}`;
    if (key === loadedKey && Date.now() - loadedAt < STALE_MS) return;
    loadedKey = key;
    loadedAt = Date.now();
    const id = ++request;
    void Bridge.calendarEvents(since, until).then((got) => {
      if (id !== request) return;
      events = got ?? [];
      render();
    });
  }

  function renderWeekdays() {
    clear(weekdays);
    const start = gridStart(month);
    for (let i = 0; i < 7; i++) {
      const name = addDays(start, i).toLocaleDateString(undefined, { weekday: "narrow" });
      weekdays.append(h("span", { text: name }));
    }
  }

  function renderGrid(start: Date) {
    clear(grid);
    const today = startOfDay(new Date());
    for (let i = 0; i < GRID_DAYS; i++) {
      const day = addDays(start, i);
      const classes = ["cal-cell"];
      if (day.getMonth() !== month.getMonth()) classes.push("other");
      if (sameDay(day, today)) classes.push("today");
      if (sameDay(day, selected)) classes.push("selected");
      if (eventsOn(events, day).length > 0) classes.push("busy");
      grid.append(
        h("button", {
          class: classes.join(" "),
          text: String(day.getDate()),
          onclick: () => {
            blip();
            selected = day;
            if (day.getMonth() !== month.getMonth()) month = new Date(day.getFullYear(), day.getMonth(), 1);
            render();
          },
        }),
      );
    }
  }

  function renderDay() {
    dayTitle.textContent = selected.toLocaleDateString(undefined, { weekday: "long", month: "long", day: "numeric" });
    clear(list);
    const todays = eventsOn(events, selected);
    if (todays.length === 0) {
      list.append(h("div", { class: "cal-empty", text: "No events" }));
      return;
    }
    for (const e of todays) {
      list.append(
        h("div", { class: "cal-event" },
          h("span", { class: "cal-event-time", text: eventTime(e) }),
          h("span", { class: "cal-event-summary", text: e.summary || "(No title)" }),
        ),
      );
    }
  }

  function render() {
    const start = gridStart(month);
    title.textContent = month.toLocaleDateString(undefined, { month: "long", year: "numeric" });
    renderWeekdays();
    renderGrid(start);
    renderDay();
    load(start);
  }

  return {
    el,
    sync() {},
    // Opening the calendar always lands on today, as GNOME's clock menu does.
    shown() {
      month = startOfDay(new Date());
      selected = startOfDay(new Date());
      render();
    },
  };
}

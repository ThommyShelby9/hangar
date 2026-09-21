/** "45 min", "2 h 05", "128 h" — play time as people say it. */
export function formatDuration(secs: number): string {
  const minutes = Math.round(secs / 60);
  if (minutes < 60) return `${minutes} min`;
  const hours = Math.floor(minutes / 60);
  if (hours >= 100) return `${hours} h`;
  const rest = minutes % 60;
  return rest === 0 ? `${hours} h` : `${hours} h ${String(rest).padStart(2, "0")}`;
}

const dateTime = new Intl.DateTimeFormat("fr-FR", { dateStyle: "long", timeStyle: "short" });
const shortDate = new Intl.DateTimeFormat("fr-FR", { day: "numeric", month: "short" });
const longDay = new Intl.DateTimeFormat("fr-FR", {
  weekday: "long",
  day: "numeric",
  month: "long",
});

export function formatDateTime(secs: number) {
  return dateTime.format(new Date(secs * 1000));
}

export function formatShortDate(secs: number) {
  return shortDate.format(new Date(secs * 1000));
}

/** "2026-09-21" → "lundi 21 septembre". */
export function formatDay(day: string) {
  return longDay.format(parseDay(day));
}

/** Local date from "YYYY-MM-DD". */
export function parseDay(day: string) {
  const [y, m, d] = day.split("-").map(Number);
  return new Date(y, m - 1, d);
}

/** Local "YYYY-MM-DD" key of a date. */
export function dayKey(date: Date) {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

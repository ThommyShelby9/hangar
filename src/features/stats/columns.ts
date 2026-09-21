import type { StatsOverview } from "@/lib/api";
import { dayKey, formatDay, parseDay } from "@/lib/format";

export interface Column {
  key: string;
  label: string;
  /** Tooltip heading. */
  title: string;
  secs: number;
}

export const monthLabel = new Intl.DateTimeFormat("fr-FR", { month: "short" });
const monthTitle = new Intl.DateTimeFormat("fr-FR", { month: "long", year: "numeric" });
const weekday = new Intl.DateTimeFormat("fr-FR", { weekday: "short" });

/** One column per day for short periods, per month otherwise. */
export function buildColumns(daily: StatsOverview["daily"], days: number | null): Column[] {
  const byDay = new Map(daily.map((d) => [d.day, d.secs]));
  const today = new Date();

  if (days !== null && days <= 31) {
    return Array.from({ length: days }, (_, i) => {
      const date = new Date(
        today.getFullYear(),
        today.getMonth(),
        today.getDate() - (days - 1 - i),
      );
      const key = dayKey(date);
      return {
        key,
        label: days <= 7 ? weekday.format(date) : String(date.getDate()),
        title: formatDay(key),
        secs: byDay.get(key) ?? 0,
      };
    });
  }

  const byMonth = new Map<string, number>();
  for (const d of daily)
    byMonth.set(d.day.slice(0, 7), (byMonth.get(d.day.slice(0, 7)) ?? 0) + d.secs);
  const first = days === null && daily.length ? parseDay(daily[0].day) : null;
  const months = first
    ? Math.max(
        1,
        (today.getFullYear() - first.getFullYear()) * 12 + today.getMonth() - first.getMonth() + 1,
      )
    : 12;
  return Array.from({ length: months }, (_, i) => {
    const date = new Date(today.getFullYear(), today.getMonth() - (months - 1 - i), 1);
    const key = dayKey(date).slice(0, 7);
    return {
      key,
      label: monthLabel.format(date).replace(".", ""),
      title: monthTitle.format(date),
      secs: byMonth.get(key) ?? 0,
    };
  });
}

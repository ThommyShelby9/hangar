import { cn } from "@/lib/utils";

const START = 135; // degrees, measured clockwise from 3 o'clock
const SWEEP = 270;

function point(cx: number, cy: number, r: number, deg: number) {
  const rad = (deg * Math.PI) / 180;
  return [cx + r * Math.cos(rad), cy + r * Math.sin(rad)];
}

function arc(cx: number, cy: number, r: number, from: number, to: number) {
  const [x1, y1] = point(cx, cy, r, from);
  const [x2, y2] = point(cx, cy, r, to);
  const large = to - from > 180 ? 1 : 0;
  return `M ${x1} ${y1} A ${r} ${r} 0 ${large} 1 ${x2} ${y2}`;
}

/**
 * Cockpit-style dial: a 270° track with a tick every 10 %, filled up to `value` (0–100).
 * The fill is the only coloured mark; the numbers stay in text colours.
 */
export function Gauge({
  value,
  label,
  unit = "%",
  dimmed,
  className,
}: {
  value: number | null;
  label: string;
  unit?: string;
  dimmed?: boolean;
  className?: string;
}) {
  const size = 148;
  const c = size / 2;
  const r = 60;
  const clamped = Math.max(0, Math.min(100, value ?? 0));
  const end = START + (SWEEP * clamped) / 100;

  return (
    <figure className={cn("relative mx-auto w-[148px]", className)}>
      <svg
        viewBox={`0 0 ${size} ${size}`}
        className="block"
        role="meter"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={value ?? undefined}
      >
        {Array.from({ length: 11 }, (_, i) => {
          const deg = START + (SWEEP * i) / 10;
          const [x1, y1] = point(c, c, r + 8, deg);
          const [x2, y2] = point(c, c, r + (i % 5 === 0 ? 14 : 11), deg);
          return (
            <line
              key={i}
              x1={x1}
              y1={y1}
              x2={x2}
              y2={y2}
              className="stroke-alu-dim/50"
              strokeWidth={i % 5 === 0 ? 1.5 : 1}
            />
          );
        })}
        <path
          d={arc(c, c, r, START, START + SWEEP)}
          className="stroke-rivet"
          strokeWidth="6"
          fill="none"
          strokeLinecap="round"
        />
        {clamped > 0.5 && !dimmed && (
          <path
            d={arc(c, c, r, START, end)}
            className="stroke-signal transition-[d] duration-500"
            strokeWidth="6"
            fill="none"
            strokeLinecap="round"
          />
        )}
      </svg>
      <div className="absolute inset-0 flex flex-col items-center justify-center pt-1">
        <span className={cn("text-[1.9rem] leading-none font-semibold", dimmed && "text-alu-dim")}>
          {value === null || dimmed ? "—" : Math.round(value)}
          {value !== null && !dimmed && (
            <span className="ml-0.5 text-lead text-alu-dim">{unit}</span>
          )}
        </span>
      </div>
      <figcaption className="-mt-3 text-center text-caption text-alu-dim">{label}</figcaption>
    </figure>
  );
}

/** The last minute of one series: a 2px line over a faint wash, fixed 0–`max` scale. */
export function Sparkline({
  values,
  max = 100,
  label,
}: {
  values: number[];
  max?: number;
  label: string;
}) {
  const width = 240;
  const height = 44;
  if (values.length < 2) return <div style={{ height }} aria-hidden />;

  const step = width / 59;
  const offset = width - step * (values.length - 1);
  const y = (v: number) => height - 1 - (Math.min(v, max) / max) * (height - 2);
  const points = values.map((v, i) => `${offset + i * step},${y(v)}`);
  const line = `M ${points.join(" L ")}`;
  const area = `${line} L ${width},${height} L ${offset},${height} Z`;

  return (
    <svg
      viewBox={`0 0 ${width} ${height}`}
      preserveAspectRatio="none"
      className="h-11 w-full"
      role="img"
      aria-label={label}
    >
      <line
        x1="0"
        y1={height - 0.5}
        x2={width}
        y2={height - 0.5}
        className="stroke-rivet"
        strokeWidth="1"
      />
      <path d={area} className="fill-signal/10" />
      <path
        d={line}
        className="stroke-signal"
        strokeWidth="2"
        fill="none"
        strokeLinejoin="round"
        strokeLinecap="round"
        vectorEffect="non-scaling-stroke"
      />
    </svg>
  );
}

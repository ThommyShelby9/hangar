import { Link } from "react-router";
import { useHardwareSnapshot } from "@/features/hardware/queries";

/** Compact live readout at the bottom of the navigation: CPU, busiest GPU, memory. */
export function SidebarMonitor() {
  const { data: snap } = useHardwareSnapshot(3000);
  if (!snap) return null;

  const gpu = snap.gpus.reduce<(typeof snap.gpus)[number] | null>(
    (best, g) => (g.usage !== null && (best === null || g.usage > (best.usage ?? 0)) ? g : best),
    null,
  );
  const rows = [
    {
      label: "CPU",
      value: snap.cpu.usage,
      detail: snap.cpu.packageC !== null ? `${snap.cpu.packageC}°` : null,
    },
    {
      label: "GPU",
      value: gpu?.usage ?? 0,
      detail: gpu?.temperatureC != null ? `${gpu.temperatureC}°` : null,
    },
    { label: "RAM", value: (snap.memoryUsed / snap.memoryTotal) * 100, detail: null },
  ];

  return (
    <Link
      to="/materiel"
      aria-label="Voir le matériel"
      className="mb-3 block space-y-2 rounded-control px-4 py-3 transition-colors hover:bg-rivet/25"
    >
      {rows.map((row) => (
        <div
          key={row.label}
          className="grid grid-cols-[2.25rem_1fr_3.25rem] items-center gap-2 text-caption"
        >
          <span className="text-alu-dim">{row.label}</span>
          <span className="h-1 rounded-full bg-rivet">
            <span
              className="block h-full rounded-full bg-signal transition-[width] duration-700"
              style={{ width: `${Math.min(100, row.value)}%` }}
            />
          </span>
          <span className="text-right text-alu-dim tabular-nums">
            {row.detail ?? `${Math.round(row.value)} %`}
          </span>
        </div>
      ))}
    </Link>
  );
}

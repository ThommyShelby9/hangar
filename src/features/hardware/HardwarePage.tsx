import type { ReactNode } from "react";
import { useMutation } from "@tanstack/react-query";
import { BatteryCharging, BatteryMedium, Moon, Thermometer } from "lucide-react";
import { hardwareApi, type GpuReading, type HardwareSnapshot } from "@/lib/api";
import { formatBytes, formatRate } from "@/lib/format";
import { toast } from "@/lib/toast";
import { cn } from "@/lib/utils";
import { PageHeader } from "@/components/PageHeader";
import { Button } from "@/components/Button";
import { Gauge } from "@/features/hardware/Gauge";
import { Sparkline } from "@/features/hardware/Sparkline";
import { useHardwareInfo, useHardwareSnapshot, useHistory } from "@/features/hardware/queries";

/** Series kept for the sparklines. */
const pickSeries = (s: HardwareSnapshot) => ({
  cpu: s.cpu.usage,
  ...Object.fromEntries(s.gpus.map((g, i) => [`gpu${i}`, g.usage])),
  memory: (s.memoryUsed / s.memoryTotal) * 100,
});

export function HardwarePage() {
  const { data: info } = useHardwareInfo();
  const { data: snap } = useHardwareSnapshot(1000);
  const history = useHistory(snap, pickSeries);

  return (
    <>
      <PageHeader
        title="Matériel"
        actions={snap?.battery && <BatteryChip battery={snap.battery} />}
      >
        {info &&
          [
            info.cpuName.replace(/\(R\)|\(TM\)|CPU /g, "").replace(/\s+/g, " "),
            `${info.physicalCores ?? "?"} cœurs, ${info.logicalCores} threads`,
            `${formatBytes(info.memoryTotal, 0)} de mémoire`,
            info.os,
          ]
            .filter(Boolean)
            .join(" · ")}
      </PageHeader>

      {snap && (
        <div className="space-y-12 px-10 pb-12">
          <div className="grid grid-cols-[repeat(auto-fill,minmax(15rem,1fr))] gap-4">
            <Panel title="Processeur">
              <Gauge value={snap.cpu.usage} label="Charge" />
              <Readouts>
                <Readout
                  label="Température"
                  value={cpuTemperature(snap)}
                  status={tempStatus(snap.cpu.packageC)}
                />
                <Readout
                  label="Consommation"
                  value={snap.cpu.powerW !== null ? `${snap.cpu.powerW.toFixed(1)} W` : "—"}
                />
                <Readout
                  label="Fréquence"
                  value={`${(snap.cpu.frequencyMhz / 1000).toFixed(1)} GHz`}
                />
              </Readouts>
              <Sparkline
                values={history.cpu ?? []}
                label="Charge du processeur sur la dernière minute"
              />
            </Panel>

            {snap.gpus.map((gpu, i) => (
              <GpuPanel key={gpu.name} gpu={gpu} history={history[`gpu${i}`] ?? []} />
            ))}

            <Panel title="Mémoire">
              <Gauge value={(snap.memoryUsed / snap.memoryTotal) * 100} label="Utilisée" />
              <Readouts>
                <Readout label="Utilisée" value={formatBytes(snap.memoryUsed)} />
                <Readout label="Libre" value={formatBytes(snap.memoryTotal - snap.memoryUsed)} />
              </Readouts>
              <Sparkline
                values={history.memory ?? []}
                label="Mémoire utilisée sur la dernière minute"
              />
            </Panel>
          </div>

          <CpuSensor snap={snap} />

          <Section title="Cœurs">
            <ul className="grid max-w-3xl grid-cols-[repeat(auto-fill,minmax(5.5rem,1fr))] gap-3">
              {snap.cpu.perCore.map((load, i) => {
                const temp = snap.cpu.coresC[i];
                return (
                  <li key={i} className="rounded-control bg-bay px-3 py-2.5">
                    <p className="text-caption text-alu-dim">Thread {i + 1}</p>
                    <p className="mt-1 font-semibold tabular-nums">{Math.round(load)} %</p>
                    <div className="mt-2 h-1.5 rounded-full bg-rivet">
                      <div
                        className="h-full rounded-full bg-signal"
                        style={{ width: `${Math.min(100, load)}%` }}
                      />
                    </div>
                    {temp != null && (
                      <p className="mt-1.5 text-caption text-alu-dim tabular-nums">{temp} °C</p>
                    )}
                  </li>
                );
              })}
            </ul>
          </Section>

          <div className="grid max-w-5xl gap-12 lg:grid-cols-[3fr_2fr]">
            {info && (
              <Section title="Disques">
                <ul className="space-y-4">
                  {info.disks
                    .slice()
                    .sort((a, b) => a.mount.localeCompare(b.mount))
                    .map((d) => {
                      const used = 1 - d.available / d.total;
                      const low = d.available / d.total < 0.05;
                      return (
                        <li key={d.mount}>
                          <div className="flex items-baseline justify-between gap-4">
                            <span>
                              {d.mount.replace("\\", "")}{" "}
                              {d.name && <span className="text-alu-dim">{d.name}</span>}
                              <span className="ml-2 text-caption text-alu-dim uppercase">
                                {d.kind === "other" ? "" : d.kind}
                              </span>
                            </span>
                            <span
                              className={cn("tabular-nums", low ? "text-flare" : "text-alu-dim")}
                            >
                              {formatBytes(d.available)} libres sur {formatBytes(d.total, 0)}
                            </span>
                          </div>
                          <div className="mt-1.5 h-2 rounded-full bg-rivet">
                            <div
                              className={cn(
                                "h-full rounded-full",
                                low ? "bg-flare" : "bg-alu-dim/70",
                              )}
                              style={{ width: `${used * 100}%` }}
                            />
                          </div>
                          {low && <p className="mt-1 text-caption text-flare">Presque plein</p>}
                        </li>
                      );
                    })}
                </ul>
              </Section>
            )}

            <Section title="Activité">
              <dl className="grid grid-cols-2 gap-x-6 gap-y-4">
                <Figure label="Téléchargement" value={formatRate(snap.netRxBps)} />
                <Figure label="Envoi" value={formatRate(snap.netTxBps)} />
                <Figure
                  label="Lecture disque"
                  value={snap.diskReadBps !== null ? formatRate(snap.diskReadBps) : "—"}
                />
                <Figure
                  label="Écriture disque"
                  value={snap.diskWriteBps !== null ? formatRate(snap.diskWriteBps) : "—"}
                />
              </dl>
            </Section>
          </div>
        </div>
      )}
    </>
  );
}

function GpuPanel({ gpu, history }: { gpu: GpuReading; history: number[] }) {
  const title = gpu.name
    .replace(/\(R\)|\(TM\)|NVIDIA |Intel /g, "")
    .replace(/\s+/g, " ")
    .trim();
  return (
    <Panel title={title}>
      <Gauge value={gpu.usage} label={gpu.asleep ? "En veille" : "Charge"} dimmed={gpu.asleep} />
      <Readouts>
        {gpu.asleep ? (
          <p className="col-span-2 flex items-center gap-2 text-caption text-alu-dim">
            <Moon size={14} />
            Éteinte pour économiser la batterie. Elle se réveille quand un jeu l'utilise.
          </p>
        ) : (
          <>
            {gpu.temperatureC !== null && (
              <Readout
                label="Température"
                value={`${gpu.temperatureC} °C`}
                status={tempStatus(gpu.temperatureC)}
              />
            )}
            {gpu.memoryUsed !== null && gpu.memoryTotal !== null && (
              <Readout
                label="Mémoire vidéo"
                value={`${formatBytes(gpu.memoryUsed)} / ${formatBytes(gpu.memoryTotal, 0)}`}
              />
            )}
            {gpu.clockMhz !== null && <Readout label="Fréquence" value={`${gpu.clockMhz} MHz`} />}
            <Readout label="Utilisée par" value={gpu.topProcess ?? "—"} />
          </>
        )}
      </Readouts>
      <Sparkline values={history} label={`Charge de ${title} sur la dernière minute`} />
    </Panel>
  );
}

function CpuSensor({ snap }: { snap: HardwareSnapshot }) {
  const enable = useMutation({
    mutationFn: hardwareApi.enableCpuSensors,
    onSuccess: () =>
      toast.info("Accepte la demande de Windows : le capteur démarre ensuite tout seul."),
    onError: toast.error,
  });
  const { sensor } = snap.cpu;
  if (sensor.state === "running") return null;

  const text: Record<string, ReactNode> = {
    noDriver:
      "Le pilote PawnIO n'est pas installé. Sans lui, Hangar n'affiche que la sonde de la carte mère.",
    off: "Le pilote PawnIO est installé. Active le capteur pour voir la vraie température et la consommation du processeur.",
    error: sensor.state === "error" ? `Le capteur ne fonctionne pas : ${sensor.message}` : null,
  };
  return (
    <aside className="flex max-w-3xl flex-wrap items-center gap-4 rounded-panel border border-rivet bg-bay px-5 py-4">
      <Thermometer size={20} className="text-signal" />
      <p className="flex-1 text-alu-dim">{text[sensor.state]}</p>
      {sensor.state !== "noDriver" && (
        <Button variant="primary" disabled={enable.isPending} onClick={() => enable.mutate()}>
          {sensor.state === "error" ? "Réessayer" : "Activer le capteur"}
        </Button>
      )}
    </aside>
  );
}

function cpuTemperature(snap: HardwareSnapshot) {
  if (snap.cpu.packageC !== null) return `${snap.cpu.packageC} °C`;
  if (snap.systemTempC !== null) return `${snap.systemTempC} °C (carte mère)`;
  return "—";
}

function tempStatus(celsius: number | null) {
  if (celsius === null) return undefined;
  if (celsius >= 90) return { label: "Critique", className: "bg-flare" };
  if (celsius >= 80) return { label: "Chaud", className: "bg-signal" };
  return { label: "Normal", className: "bg-primer" };
}

function BatteryChip({ battery }: { battery: { percent: number; pluggedIn: boolean } }) {
  const Icon = battery.pluggedIn ? BatteryCharging : BatteryMedium;
  return (
    <p className="flex items-center gap-2 rounded-full border border-rivet px-3.5 py-1.5 text-alu-dim">
      <Icon size={16} />
      <span className="text-alu tabular-nums">{battery.percent} %</span>
      {battery.pluggedIn ? "sur secteur" : "sur batterie"}
    </p>
  );
}

function Panel({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-4 rounded-panel border border-rivet bg-bay/60 p-5">
      <h2 className="truncate font-display text-[1.25rem] font-semibold">{title}</h2>
      {children}
    </section>
  );
}

function Readouts({ children }: { children: ReactNode }) {
  return <dl className="grid grid-cols-2 gap-x-4 gap-y-3">{children}</dl>;
}

function Readout({
  label,
  value,
  status,
}: {
  label: string;
  value: string;
  status?: { label: string; className: string };
}) {
  return (
    <div className="min-w-0">
      <dt className="text-caption text-alu-dim">{label}</dt>
      <dd className="truncate tabular-nums">{value}</dd>
      {status && (
        <dd className="mt-0.5 flex items-center gap-1.5 text-caption text-alu-dim">
          <span className={cn("size-1.5 rounded-full", status.className)} />
          {status.label}
        </dd>
      )}
    </div>
  );
}

function Figure({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="text-alu-dim">{label}</dt>
      <dd className="mt-0.5 text-[1.25rem] font-semibold tabular-nums">{value}</dd>
    </div>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section>
      <h2 className="mb-5 font-display text-title font-semibold">{title}</h2>
      {children}
    </section>
  );
}

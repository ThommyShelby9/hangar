import { useEffect, useRef, useState, type ReactNode } from "react";
import { BatteryCharging, BatteryMedium, Bluetooth, Cable, Info } from "lucide-react";
import { controllersApi, type PadInfo, type PadInput } from "@/lib/api";
import { toast } from "@/lib/toast";
import { cn } from "@/lib/utils";
import { PageHeader } from "@/components/PageHeader";
import { EmptyState } from "@/components/EmptyState";
import { Button } from "@/components/Button";
import { PadDiagram } from "@/features/controllers/PadDiagram";
import { useControllers, useLiveInputs } from "@/features/controllers/queries";

const KIND_LABEL: Record<PadInfo["kind"], string> = {
  ps4: "DualShock 4",
  ps3: "DualShock 3",
  ps5: "DualSense",
  xbox: "Manette Xbox",
  switch: "Manette Switch",
  other: "Manette",
};

export function ControllersPage() {
  const { data: status } = useControllers();
  const inputs = useLiveInputs();
  const [selected, setSelected] = useState<number | null>(null);
  const pads = status?.pads ?? [];
  const pad = pads.find((p) => p.id === selected) ?? pads[0];

  return (
    <>
      <PageHeader title="Manettes" />
      <div className="space-y-8 px-10 pb-12">
        {status?.ds4windows && (
          <p className="flex max-w-3xl items-start gap-3 rounded-panel border border-rivet bg-bay px-5 py-4 text-alu-dim">
            <Info size={18} className="mt-0.5 shrink-0 text-signal" />
            DS4Windows est ouvert : il garde la main sur la barre lumineuse et l'émulation Xbox.
            Hangar se contente de lire la manette. Si seule une « manette Xbox » apparaît ici, c'est
            celle que DS4Windows crée ; ajoute Hangar à la liste des applications autorisées de
            HidHide pour voir la vraie DS4.
          </p>
        )}

        {!pad ? (
          <EmptyState title="Aucune manette connectée">
            Allume ta manette avec le bouton PS, ou branche-la en USB. Elle apparaîtra ici toute
            seule.
          </EmptyState>
        ) : (
          <>
            {pads.length > 1 && (
              <div role="tablist" className="flex flex-wrap gap-2">
                {pads.map((p) => (
                  <button
                    key={p.id}
                    role="tab"
                    aria-selected={p.id === pad.id}
                    onClick={() => setSelected(p.id)}
                    className={cn(
                      "h-9 rounded-control px-4",
                      p.id === pad.id ? "bg-rivet/60 text-alu" : "text-alu-dim hover:bg-rivet/30",
                    )}
                  >
                    {KIND_LABEL[p.kind]}
                  </button>
                ))}
              </div>
            )}
            <PadView pad={pad} input={inputs[pad.id]} />
          </>
        )}
      </div>
    </>
  );
}

function PadView({ pad, input }: { pad: PadInfo; input?: PadInput }) {
  return (
    <div className="grid gap-10 xl:grid-cols-[minmax(0,32rem)_1fr]">
      <section className="space-y-4">
        <header>
          <h2 className="font-display text-[1.75rem] leading-tight font-semibold">
            {KIND_LABEL[pad.kind]}
          </h2>
          <p className="mt-1 flex flex-wrap items-center gap-x-4 gap-y-1 text-alu-dim">
            <span>{pad.name}</span>
            <Connection pad={pad} />
            <PadBattery pad={pad} />
          </p>
        </header>
        <PadDiagram pad={pad} input={input} />
        {!input && (
          <p className="text-caption text-alu-dim">
            Appuie sur un bouton pour vérifier la liaison.
          </p>
        )}
      </section>

      <div className="space-y-8">
        <Block title="Gâchettes">
          <div className="grid max-w-md grid-cols-2 gap-4">
            <Meter label="L2" value={input?.triggers[0] ?? 0} />
            <Meter label="R2" value={input?.triggers[1] ?? 0} />
          </div>
        </Block>

        <Block title="Sticks">
          <div className="grid max-w-md grid-cols-2 gap-4">
            <StickReadout label="Gauche" value={input?.left} />
            <StickReadout label="Droit" value={input?.right} />
          </div>
          <DriftTest input={input} />
        </Block>

        {pad.hasRumble && (
          <Block title="Vibrations">
            <div className="flex flex-wrap gap-2">
              <Button onClick={() => controllersApi.rumble(pad.id, true).catch(toast.error)}>
                Moteur gauche (fort)
              </Button>
              <Button onClick={() => controllersApi.rumble(pad.id, false).catch(toast.error)}>
                Moteur droit (léger)
              </Button>
            </div>
          </Block>
        )}

        {pad.hasMotion && <Motion pad={pad} input={input} />}
      </div>
    </div>
  );
}

/** Leave the sticks alone for two seconds: any movement measured is drift. */
function DriftTest({ input }: { input?: PadInput }) {
  const [result, setResult] = useState<{ left: number; right: number } | null>(null);
  const [running, setRunning] = useState(false);
  const peak = useRef({ left: 0, right: 0 });

  useEffect(() => {
    if (!running || !input) return;
    peak.current.left = Math.max(peak.current.left, Math.hypot(...input.left));
    peak.current.right = Math.max(peak.current.right, Math.hypot(...input.right));
  }, [running, input]);

  const start = () => {
    peak.current = { left: 0, right: 0 };
    setResult(null);
    setRunning(true);
    setTimeout(() => {
      setRunning(false);
      setResult({ ...peak.current });
    }, 2000);
  };

  const verdict = (v: number) =>
    v >= 0.1 ? "dérive nette" : v >= 0.05 ? "léger jeu, sans conséquence" : "aucune dérive";

  return (
    <div className="mt-4 flex flex-wrap items-center gap-4">
      <Button onClick={start} disabled={running || !input}>
        {running ? "Ne touche pas aux sticks…" : "Tester la dérive"}
      </Button>
      {result && (
        <p className="text-alu-dim">
          Gauche : {verdict(result.left)} ({Math.round(result.left * 100)} %) · Droit :{" "}
          {verdict(result.right)} ({Math.round(result.right * 100)} %)
        </p>
      )}
    </div>
  );
}

function Motion({ pad, input }: { pad: PadInfo; input?: PadInput }) {
  const [enabled, setEnabled] = useState(false);
  const toggle = () => {
    controllersApi.motion(pad.id, !enabled).catch(toast.error);
    setEnabled(!enabled);
  };
  return (
    <Block title="Capteurs de mouvement">
      {enabled && input?.gyro ? (
        <div className="grid max-w-md grid-cols-3 gap-4">
          {(["X", "Y", "Z"] as const).map((axis, i) => (
            <Meter
              key={axis}
              label={`Gyro ${axis}`}
              value={Math.min(1, Math.abs(input.gyro![i]) / 6)}
            />
          ))}
        </div>
      ) : (
        <p className="max-w-[60ch] text-alu-dim">
          Le gyroscope passe la manette dans le mode avancé de SDL. En Bluetooth, certains jeux ne
          la reconnaissent plus ensuite, jusqu'à ce que tu l'éteignes et la rallumes.
        </p>
      )}
      <Button className="mt-3" onClick={toggle}>
        {enabled ? "Couper le gyroscope" : "Activer le gyroscope"}
      </Button>
    </Block>
  );
}

function StickReadout({ label, value }: { label: string; value?: [number, number] }) {
  const [x, y] = value ?? [0, 0];
  return (
    <div className="flex items-center gap-3">
      <svg viewBox="-1.1 -1.1 2.2 2.2" className="size-16" aria-hidden>
        <circle r={1} className="fill-bay stroke-rivet" strokeWidth={0.04} />
        <line x1={-1} x2={1} y1={0} y2={0} className="stroke-rivet" strokeWidth={0.02} />
        <line y1={-1} y2={1} x1={0} x2={0} className="stroke-rivet" strokeWidth={0.02} />
        <circle cx={x} cy={y} r={0.14} className="fill-signal" />
      </svg>
      <div className="text-caption">
        <p className="text-alu-dim">{label}</p>
        <p className="tabular-nums">
          {x.toFixed(2)} / {y.toFixed(2)}
        </p>
      </div>
    </div>
  );
}

function Meter({ label, value }: { label: string; value: number }) {
  return (
    <div>
      <div className="flex justify-between text-caption">
        <span className="text-alu-dim">{label}</span>
        <span className="tabular-nums">{Math.round(value * 100)} %</span>
      </div>
      <div className="mt-1.5 h-2 rounded-full bg-rivet">
        <div
          className="h-full rounded-full bg-signal"
          style={{ width: `${Math.max(0, Math.min(1, value)) * 100}%` }}
        />
      </div>
    </div>
  );
}

function Connection({ pad }: { pad: PadInfo }) {
  if (pad.connection === "unknown") return null;
  const Icon = pad.connection === "usb" ? Cable : Bluetooth;
  return (
    <span className="flex items-center gap-1.5">
      <Icon size={14} />
      {pad.connection === "usb" ? "USB" : "Bluetooth"}
    </span>
  );
}

function PadBattery({ pad }: { pad: PadInfo }) {
  if (!pad.battery) return null;
  const Icon = pad.battery.charging ? BatteryCharging : BatteryMedium;
  return (
    <span className="flex items-center gap-1.5">
      <Icon size={14} />
      {pad.battery.percent !== null ? `${pad.battery.percent} %` : "Batterie"}
      {pad.battery.charging && " · en charge"}
    </span>
  );
}

function Block({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section>
      <h3 className="mb-3 font-display text-[1.25rem] font-semibold">{title}</h3>
      {children}
    </section>
  );
}

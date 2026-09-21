import type { PadInfo, PadInput } from "@/lib/api";
import { cn } from "@/lib/utils";

const PLAYSTATION = { south: "✕", east: "○", west: "□", north: "△" };
const XBOX = { south: "A", east: "B", west: "X", north: "Y" };

/** Top-down controller drawing; pressed buttons light up in the signal colour. */
export function PadDiagram({ pad, input }: { pad: PadInfo; input?: PadInput }) {
  const pressed = new Set(input?.buttons ?? []);
  const on = (name: string) =>
    pressed.has(name) ? "fill-signal stroke-signal" : "fill-bay stroke-alu-dim/60";
  const labels = pad.kind.startsWith("ps") ? PLAYSTATION : XBOX;
  const faceLabel = (name: keyof typeof PLAYSTATION) =>
    pressed.has(name) ? "fill-signal-ink" : "fill-alu";

  const face = [
    { name: "north" as const, x: 302, y: 84 },
    { name: "south" as const, x: 302, y: 134 },
    { name: "west" as const, x: 277, y: 109 },
    { name: "east" as const, x: 327, y: 109 },
  ];
  const stick = (name: "l3" | "r3", cx: number, value: [number, number] | undefined) => (
    <g>
      <circle cx={cx} cy={168} r={24} className={cn("stroke-[1.5]", on(name))} />
      <circle
        cx={cx + (value?.[0] ?? 0) * 12}
        cy={168 + (value?.[1] ?? 0) * 12}
        r={11}
        className={pressed.has(name) ? "fill-signal-ink" : "fill-alu-dim/70"}
      />
    </g>
  );

  return (
    <svg
      viewBox="0 0 400 270"
      className="w-full max-w-lg"
      role="img"
      aria-label={`Schéma de la manette ${pad.name}`}
    >
      {/* Shoulder buttons */}
      <rect x={70} y={22} width={70} height={14} rx={7} className={cn("stroke-[1.5]", on("l1"))} />
      <rect x={260} y={22} width={70} height={14} rx={7} className={cn("stroke-[1.5]", on("r1"))} />
      {/* Body */}
      <path
        d="M110 42 H290 C332 42 352 58 364 94 L394 204 C402 240 372 262 346 246 L300 206 H100 L54 246 C28 262 -2 240 6 204 L36 94 C48 58 68 42 110 42 Z"
        className="fill-tarmac stroke-rivet"
        strokeWidth={2}
      />
      {/* Touchpad */}
      {pad.hasTouchpad && (
        <g>
          <rect
            x={148}
            y={52}
            width={104}
            height={62}
            rx={8}
            className={cn("stroke-[1.5]", on("touchpad"))}
          />
          {input?.fingers.map(([x, y], i) => (
            <circle key={i} cx={148 + x * 104} cy={52 + y * 62} r={6} className="fill-primer" />
          ))}
        </g>
      )}
      {/* Share / Options */}
      <rect
        x={124}
        y={60}
        width={10}
        height={20}
        rx={5}
        className={cn("stroke-[1.5]", on("share"))}
      />
      <rect
        x={266}
        y={60}
        width={10}
        height={20}
        rx={5}
        className={cn("stroke-[1.5]", on("options"))}
      />
      {/* D-pad */}
      <rect x={90} y={80} width={20} height={22} rx={3} className={cn("stroke-[1.5]", on("up"))} />
      <rect
        x={90}
        y={116}
        width={20}
        height={22}
        rx={3}
        className={cn("stroke-[1.5]", on("down"))}
      />
      <rect
        x={66}
        y={99}
        width={22}
        height={20}
        rx={3}
        className={cn("stroke-[1.5]", on("left"))}
      />
      <rect
        x={112}
        y={99}
        width={22}
        height={20}
        rx={3}
        className={cn("stroke-[1.5]", on("right"))}
      />
      {/* Face buttons */}
      {face.map((b) => (
        <g key={b.name}>
          <circle cx={b.x} cy={b.y} r={13} className={cn("stroke-[1.5]", on(b.name))} />
          <text
            x={b.x}
            y={b.y + 5}
            textAnchor="middle"
            className={cn("text-[13px] font-semibold", faceLabel(b.name))}
          >
            {labels[b.name]}
          </text>
        </g>
      ))}
      {stick("l3", 150, input?.left)}
      {stick("r3", 250, input?.right)}
      {/* Home button */}
      <circle cx={200} cy={150} r={9} className={cn("stroke-[1.5]", on("ps"))} />
    </svg>
  );
}

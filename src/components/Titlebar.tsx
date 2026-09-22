import { useEffect, useState, type ReactNode } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Copy, Minus, Square, Tv, X } from "lucide-react";
import { useNavigate } from "react-router";
import { cn } from "@/lib/utils";

const appWindow = getCurrentWindow();

export function Titlebar() {
  const navigate = useNavigate();
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    const sync = () => appWindow.isMaximized().then(setMaximized);
    sync();
    const unlisten = appWindow.onResized(sync);
    return () => {
      unlisten.then((off) => off());
    };
  }, []);

  return (
    <header data-tauri-drag-region className="flex h-10 shrink-0 items-center justify-between pl-5">
      <span
        data-tauri-drag-region
        className="flex items-center gap-2 font-stencil text-[1.3rem] font-bold tracking-[0.08em] text-alu"
      >
        <HangarMark />
        HANGAR
      </span>

      <div className="flex h-full items-center">
        <button
          type="button"
          onClick={() => navigate("/tv")}
          title="Plein écran, à piloter à la manette"
          className="mr-3 flex h-7 items-center gap-2 rounded-control px-2.5 text-caption text-alu-dim transition-colors hover:bg-rivet/60 hover:text-alu"
        >
          <Tv size={14} />
          Mode TV
        </button>
        <WindowButton label="Réduire" onClick={() => appWindow.minimize()}>
          <Minus size={16} strokeWidth={1.5} />
        </WindowButton>
        <WindowButton
          label={maximized ? "Restaurer" : "Agrandir"}
          onClick={() => appWindow.toggleMaximize()}
        >
          {maximized ? (
            <Copy size={13} strokeWidth={1.5} className="-scale-x-100" />
          ) : (
            <Square size={13} strokeWidth={1.5} />
          )}
        </WindowButton>
        <WindowButton label="Fermer" onClick={() => appWindow.close()} danger>
          <X size={17} strokeWidth={1.5} />
        </WindowButton>
      </div>
    </header>
  );
}

function WindowButton({
  label,
  onClick,
  danger,
  children,
}: {
  label: string;
  onClick: () => void;
  danger?: boolean;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      className={cn(
        "grid w-12 place-items-center self-stretch text-alu-dim transition-colors hover:text-alu focus-visible:-outline-offset-2",
        danger ? "hover:bg-flare hover:text-white" : "hover:bg-rivet/60",
      )}
    >
      {children}
    </button>
  );
}

/** The app icon's hangar, without its tile: an amber arch, its open door and the apron. */
function HangarMark() {
  return (
    <svg viewBox="160 240 704 400" aria-hidden className="pointer-events-none h-3.5 w-auto">
      <path d="M176 606 A336 360 0 0 1 848 606 Z" className="fill-signal" />
      <path d="M384 606 V434 Q384 414 404 414 H620 Q640 414 640 434 V606 Z" className="fill-tarmac" />
      <rect x="160" y="606" width="704" height="28" rx="14" className="fill-alu" />
    </svg>
  );
}

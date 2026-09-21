import { useEffect, useState, type ReactNode } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Copy, Minus, Square, X } from "lucide-react";
import { cn } from "@/lib/utils";

const appWindow = getCurrentWindow();

export function Titlebar() {
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
        className="font-stencil text-[1.3rem] font-bold tracking-[0.08em] text-alu"
      >
        HANGAR
      </span>

      <div className="flex h-full">
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
        "grid w-12 place-items-center text-alu-dim transition-colors hover:text-alu focus-visible:-outline-offset-2",
        danger ? "hover:bg-flare hover:text-white" : "hover:bg-rivet/60",
      )}
    >
      {children}
    </button>
  );
}

import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";
import { useNavigate } from "react-router";
import Fuse from "fuse.js";
import {
  Activity,
  Cpu,
  Gamepad2,
  LayoutGrid,
  Play,
  Search,
  Settings,
  type LucideIcon,
} from "lucide-react";
import { api } from "@/lib/api";
import { toast } from "@/lib/toast";
import { cn } from "@/lib/utils";
import { useGames } from "@/features/library/queries";

interface Item {
  key: string;
  label: string;
  hint: string;
  icon: LucideIcon;
  run: (launch: boolean) => void;
}

const PAGES: { to: string; label: string; icon: LucideIcon }[] = [
  { to: "/bibliotheque", label: "Bibliothèque", icon: LayoutGrid },
  { to: "/stats", label: "Stats", icon: Activity },
  { to: "/materiel", label: "Matériel", icon: Cpu },
  { to: "/manettes", label: "Manettes", icon: Gamepad2 },
  { to: "/reglages", label: "Réglages", icon: Settings },
];

/** Ctrl+K from anywhere: find a game or a page. Enter opens, Shift+Enter launches the game. */
export function CommandPalette() {
  const [open, setOpen] = useState(false);
  const dialog = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setOpen((o) => !o);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    const el = dialog.current;
    if (open && el && !el.open) el.showModal();
    if (!open && el?.open) el.close();
  }, [open]);

  return (
    <dialog
      ref={dialog}
      onClose={() => setOpen(false)}
      onClick={(e) => e.target === dialog.current && setOpen(false)}
      aria-label="Palette de commandes"
      className="mx-auto mt-[12vh] w-[min(36rem,90vw)] overflow-hidden rounded-panel border border-rivet bg-tarmac p-0 text-alu shadow-2xl shadow-black/60 backdrop:bg-black/50"
    >
      {open && <Palette close={() => setOpen(false)} />}
    </dialog>
  );
}

function Palette({ close }: { close: () => void }) {
  const navigate = useNavigate();
  const { data: games = [] } = useGames();
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);

  const items = useMemo<Item[]>(() => {
    const go = (to: string) => () => {
      navigate(to);
      close();
    };
    const gameItems: Item[] = games
      .filter((g) => !g.hidden)
      .map((g) => ({
        key: `game-${g.id}`,
        label: g.title,
        hint: "Jeu",
        icon: Play,
        run: (launch) => {
          close();
          if (launch) {
            api
              .launchGame(g.id)
              .then(() => toast.info(`Lancement de ${g.title}…`))
              .catch(toast.error);
          } else {
            navigate(`/jeu/${g.id}`);
          }
        },
      }));
    const pageItems: Item[] = PAGES.map((p) => ({
      key: p.to,
      label: p.label,
      hint: "Page",
      icon: p.icon,
      run: go(p.to),
    }));
    const all = [...gameItems, ...pageItems];
    if (!query.trim()) return [...pageItems, ...gameItems].slice(0, 12);
    return new Fuse(all, { keys: ["label"], threshold: 0.4, ignoreLocation: true })
      .search(query.trim())
      .slice(0, 12)
      .map((r) => r.item);
  }, [games, query, navigate, close]);

  const current = Math.min(active, Math.max(0, items.length - 1));

  const onKeyDown = (e: ReactKeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((current + 1) % Math.max(1, items.length));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((current - 1 + items.length) % Math.max(1, items.length));
    } else if (e.key === "Enter" && items[current]) {
      e.preventDefault();
      items[current].run(e.shiftKey);
    }
  };

  return (
    <div onKeyDown={onKeyDown}>
      <label className="flex items-center gap-3 border-b border-rivet px-5">
        <Search size={18} className="text-alu-dim" />
        <input
          autoFocus
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setActive(0);
          }}
          placeholder="Chercher un jeu ou une page"
          aria-label="Chercher un jeu ou une page"
          role="combobox"
          aria-expanded
          aria-controls="palette-results"
          aria-activedescendant={items[current]?.key}
          className="h-14 flex-1 bg-transparent text-lead placeholder:text-alu-dim/60 focus:outline-none"
        />
      </label>
      <ul id="palette-results" role="listbox" className="max-h-[50vh] overflow-y-auto py-2">
        {items.length === 0 && <li className="px-5 py-3 text-alu-dim">Rien ne correspond.</li>}
        {items.map((item, i) => (
          <li
            key={item.key}
            id={item.key}
            role="option"
            aria-selected={i === current}
            onMouseMove={() => setActive(i)}
            onClick={(e) => item.run(e.shiftKey)}
            className={cn(
              "mx-2 flex cursor-default items-center gap-3 rounded-control px-3 py-2.5",
              i === current && "bg-rivet/50",
            )}
          >
            <item.icon size={16} className={i === current ? "text-signal" : "text-alu-dim"} />
            <span className="flex-1 truncate">{item.label}</span>
            <span className="text-caption text-alu-dim">{item.hint}</span>
          </li>
        ))}
      </ul>
      <p className="border-t border-rivet px-5 py-2.5 text-caption text-alu-dim">
        Entrée pour ouvrir · Maj + Entrée pour lancer le jeu · Échap pour fermer
      </p>
    </div>
  );
}

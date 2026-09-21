import { NavLink } from "react-router";
import { motion } from "motion/react";
import { Activity, Camera, Cpu, Gamepad2, LayoutGrid, Settings, type LucideIcon } from "lucide-react";
import { cn } from "@/lib/utils";
import { SidebarMonitor } from "@/components/SidebarMonitor";

interface NavItem {
  to: string;
  label: string;
  icon: LucideIcon;
}

const MAIN_NAV: NavItem[] = [
  { to: "/bibliotheque", label: "Bibliothèque", icon: LayoutGrid },
  { to: "/stats", label: "Stats", icon: Activity },
  { to: "/captures", label: "Captures", icon: Camera },
  { to: "/materiel", label: "Matériel", icon: Cpu },
  { to: "/manettes", label: "Manettes", icon: Gamepad2 },
];

const SETTINGS_NAV: NavItem = { to: "/reglages", label: "Réglages", icon: Settings };

export function Sidebar() {
  return (
    <nav aria-label="Navigation principale" className="flex w-56 shrink-0 flex-col px-3 pt-4 pb-3">
      <ul className="flex flex-col gap-0.5">
        {MAIN_NAV.map((item) => (
          <li key={item.to}>
            <SidebarLink item={item} />
          </li>
        ))}
      </ul>
      <div className="mt-auto">
        <SidebarMonitor />
        <SidebarLink item={SETTINGS_NAV} />
      </div>
    </nav>
  );
}

function SidebarLink({ item: { to, label, icon: Icon } }: { item: NavItem }) {
  return (
    <NavLink
      to={to}
      className={({ isActive }) =>
        cn(
          "relative flex h-11 items-center gap-3 rounded-control pr-3 pl-4 transition-colors",
          isActive ? "bg-rivet/45 text-alu" : "text-alu-dim hover:bg-rivet/25 hover:text-alu",
        )
      }
    >
      {({ isActive }) => (
        <>
          {isActive && (
            // Painted taxiway line: slides to the selected section.
            <motion.span
              layoutId="taxiway"
              className="absolute inset-y-2 left-0 w-[3px] rounded-full bg-signal"
              transition={{ type: "spring", stiffness: 520, damping: 40 }}
            />
          )}
          <Icon size={18} strokeWidth={1.75} className={isActive ? "text-signal" : undefined} />
          <span className="font-display text-[1.1875rem] leading-none font-semibold tracking-[0.01em]">
            {label}
          </span>
        </>
      )}
    </NavLink>
  );
}

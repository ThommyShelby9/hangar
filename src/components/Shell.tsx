import { useEffect } from "react";
import { Outlet, useNavigate } from "react-router";
import { listen } from "@tauri-apps/api/event";
import { Titlebar } from "@/components/Titlebar";
import { Sidebar } from "@/components/Sidebar";
import { Toaster } from "@/components/Toaster";
import { CommandPalette } from "@/components/CommandPalette";
import { SessionSummary } from "@/components/SessionSummary";
import { useLibraryEvents } from "@/features/library/queries";

export function Shell() {
  useLibraryEvents();
  const navigate = useNavigate();
  // "Mode TV" from the notification-area menu.
  useEffect(() => {
    const off = listen("open-tv", () => navigate("/tv"));
    return () => {
      off.then((f) => f());
    };
  }, [navigate]);
  return (
    <div className="flex h-full flex-col bg-tarmac-glass">
      <Titlebar />
      <div className="flex min-h-0 flex-1">
        <Sidebar />
        <main className="min-w-0 flex-1 overflow-y-auto rounded-tl-panel border-t border-l border-rivet bg-tarmac">
          <Outlet />
        </main>
      </div>
      <Toaster />
      <CommandPalette />
      <SessionSummary />
    </div>
  );
}

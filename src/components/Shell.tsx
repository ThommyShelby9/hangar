import { Outlet } from "react-router";
import { Titlebar } from "@/components/Titlebar";
import { Sidebar } from "@/components/Sidebar";
import { Toaster } from "@/components/Toaster";
import { useLibraryEvents } from "@/features/library/queries";

export function Shell() {
  useLibraryEvents();
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
    </div>
  );
}

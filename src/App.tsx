import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MotionConfig } from "motion/react";
import { HashRouter, Navigate, Route, Routes } from "react-router";
import { Shell } from "@/components/Shell";
import { LibraryPage } from "@/features/library/LibraryPage";
import { StatsPage } from "@/features/stats/StatsPage";
import { HardwarePage } from "@/features/hardware/HardwarePage";
import { ControllersPage } from "@/features/controllers/ControllersPage";
import { SettingsPage } from "@/features/settings/SettingsPage";

const queryClient = new QueryClient();

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <MotionConfig reducedMotion="user">
        <HashRouter>
          <Routes>
            <Route element={<Shell />}>
              <Route index element={<Navigate to="/bibliotheque" replace />} />
              <Route path="bibliotheque" element={<LibraryPage />} />
              <Route path="stats" element={<StatsPage />} />
              <Route path="materiel" element={<HardwarePage />} />
              <Route path="manettes" element={<ControllersPage />} />
              <Route path="reglages" element={<SettingsPage />} />
            </Route>
          </Routes>
        </HashRouter>
      </MotionConfig>
    </QueryClientProvider>
  );
}

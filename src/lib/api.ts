import { invoke } from "@tauri-apps/api/core";

export interface AppInfo {
  version: string;
  dataDir: string;
  schemaVersion: number;
  onSystemDrive: boolean;
}

/** Typed wrappers around the Tauri commands declared in src-tauri/src/commands.rs. */
export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
};

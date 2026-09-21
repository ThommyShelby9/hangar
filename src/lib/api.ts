import { invoke } from "@tauri-apps/api/core";

export interface AppInfo {
  version: string;
  dataDir: string;
  schemaVersion: number;
  onSystemDrive: boolean;
}

export type ArtKind = "grid" | "hero" | "logo" | "icon";

export interface Game {
  id: number;
  title: string;
  sortTitle: string;
  source: "manual" | "steam" | "epic";
  installDir: string | null;
  status: "ok" | "missing";
  favorite: boolean;
  hidden: boolean;
  sgdbId: number | null;
  addedAt: number;
  lastPlayedAt: number | null;
  /** Files relative to the cache folder, see `imageUrl`. */
  images: Partial<Record<ArtKind, string>>;
}

export interface LaunchAction {
  id: number;
  name: string;
  target: string;
  args: string;
  workingDir: string | null;
  runAsAdmin: boolean;
  isDefault: boolean;
}

export interface GameDetail extends Game {
  actions: LaunchAction[];
}

export interface ExeCandidate {
  path: string;
  size: number;
  productName: string | null;
}

export interface ExtraAction {
  name: string;
  path: string;
}

export interface FolderCandidate {
  dir: string;
  title: string;
  kind: "game" | "installer";
  exes: ExeCandidate[];
  extras: ExtraAction[];
}

export interface NewManualGame {
  dir: string;
  title: string;
  exe: string;
  extras: ExtraAction[];
}

export interface GamePatch {
  title?: string;
  favorite?: boolean;
  hidden?: boolean;
}

export interface SgdbGame {
  id: number;
  name: string;
  releaseDate: number | null;
}

export interface SgdbImage {
  id: number;
  url: string;
  thumb: string;
  width: number;
  height: number;
}

/** Emitted by the backend whenever the library changes. */
export const LIBRARY_CHANGED = "library-changed";

/** Typed wrappers around the Tauri commands declared in src-tauri/src/commands.rs. */
export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),

  listGames: () => invoke<Game[]>("list_games"),
  getGame: (id: number) => invoke<GameDetail>("get_game", { id }),
  updateGame: (id: number, patch: GamePatch) => invoke<void>("update_game", { id, patch }),
  updateAction: (action: LaunchAction) => invoke<void>("update_action", { action }),
  removeGame: (id: number) => invoke<void>("remove_game", { id }),
  launchGame: (id: number, actionId?: number) => invoke<void>("launch_game", { id, actionId }),

  scanPlatforms: () => invoke<number>("scan_platforms"),
  scanFolders: () => invoke<FolderCandidate[]>("scan_folders"),
  analyzePaths: (paths: string[]) => invoke<FolderCandidate[]>("analyze_paths", { paths }),
  importGames: (games: NewManualGame[]) => invoke<number[]>("import_games", { games }),
  ignoreFolders: (dirs: string[]) => invoke<void>("ignore_folders", { dirs }),

  watchedFolders: () => invoke<string[]>("watched_folders"),
  addWatchedFolder: (path: string) => invoke<void>("add_watched_folder", { path }),
  removeWatchedFolder: (path: string) => invoke<void>("remove_watched_folder", { path }),

  sgdbKeySet: () => invoke<boolean>("sgdb_key_set"),
  setSgdbKey: (key: string) => invoke<void>("set_sgdb_key", { key }),
  sgdbSearch: (term: string) => invoke<SgdbGame[]>("sgdb_search", { term }),
  sgdbImages: (sgdbId: number, kind: ArtKind) =>
    invoke<SgdbImage[]>("sgdb_images", { sgdbId, kind }),
  setGameSgdb: (id: number, sgdbId: number) => invoke<void>("set_game_sgdb", { id, sgdbId }),
  setGameImage: (id: number, kind: ArtKind, url: string) =>
    invoke<void>("set_game_image", { id, kind, url }),
};

/** URL of a cached artwork file, served by the backend's `hangar` protocol. */
export function imageUrl(file: string | undefined): string | undefined {
  return file ? `http://hangar.localhost/${file}` : undefined;
}

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
  playtimeSecs: number;
  /** Ids of the collections the game belongs to. */
  collections: number[];
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
  /** Arguments carried by a .lnk shortcut. */
  args: string;
}

export interface NewManualGame {
  dir: string;
  title: string;
  /** Executable, or a link such as steam://… for games added from a .url shortcut. */
  exe: string;
  args: string;
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

export interface GameStats {
  totalSecs: number;
  sessionCount: number;
  lastSession: { startedAt: number; durationSecs: number } | null;
}

export interface StatsOverview {
  totalSecs: number;
  sessionCount: number;
  gamesPlayed: number;
  /** Local dates "YYYY-MM-DD" with play time; days without play are absent. */
  daily: { day: string; secs: number }[];
  topGames: {
    gameId: number;
    title: string;
    secs: number;
    sessions: number;
    grid: string | null;
  }[];
  recent: {
    id: number;
    gameId: number;
    title: string;
    startedAt: number;
    durationSecs: number;
    running: boolean;
  }[];
}

/** Emitted by the tracker when a game starts or stops. */
export const SESSIONS_CHANGED = "sessions-changed";

export const statsApi = {
  runningGames: () => invoke<number[]>("running_games"),
  gameStats: (id: number) => invoke<GameStats>("game_stats", { id }),
  overview: (days: number | null) => invoke<StatsOverview>("stats_overview", { days }),
};

export interface HardwareInfo {
  cpuName: string;
  physicalCores: number | null;
  logicalCores: number;
  memoryTotal: number;
  os: string | null;
  motherboard: string | null;
  gpus: { name: string; vendor: string; dedicatedMemory: number }[];
  disks: {
    mount: string;
    name: string;
    kind: "ssd" | "hdd" | "other";
    total: number;
    available: number;
  }[];
}

export type SensorStatus =
  | { state: "noDriver" }
  | { state: "off" }
  | { state: "running" }
  | { state: "error"; message: string };

export interface GpuReading {
  name: string;
  vendor: string;
  usage: number | null;
  topProcess: string | null;
  temperatureC: number | null;
  memoryUsed: number | null;
  memoryTotal: number | null;
  clockMhz: number | null;
  powerW: number | null;
  asleep: boolean;
}

export interface HardwareSnapshot {
  cpu: {
    usage: number;
    perCore: number[];
    frequencyMhz: number;
    packageC: number | null;
    coresC: (number | null)[];
    powerW: number | null;
    sensor: SensorStatus;
  };
  memoryUsed: number;
  memoryTotal: number;
  gpus: GpuReading[];
  diskReadBps: number | null;
  diskWriteBps: number | null;
  netRxBps: number;
  netTxBps: number;
  battery: { percent: number; pluggedIn: boolean } | null;
  systemTempC: number | null;
}

export const hardwareApi = {
  info: () => invoke<HardwareInfo>("hardware_info"),
  snapshot: () => invoke<HardwareSnapshot>("hardware_snapshot"),
  enableCpuSensors: () => invoke<void>("enable_cpu_sensors"),
};

export interface PadInfo {
  id: number;
  name: string;
  kind: "ps4" | "ps3" | "ps5" | "xbox" | "switch" | "other";
  connection: "usb" | "bluetooth" | "unknown";
  battery: { percent: number | null; charging: boolean } | null;
  hasRumble: boolean;
  hasTouchpad: boolean;
  hasMotion: boolean;
}

export interface ControllersStatus {
  pads: PadInfo[];
  ds4windows: boolean;
}

export interface PadInput {
  id: number;
  /** Pressed buttons: south, east, west, north, l1, r1, l3, r3, share, options, ps, touchpad, up… */
  buttons: string[];
  /** Sticks, -1…1 (y grows downwards). */
  left: [number, number];
  right: [number, number];
  /** L2 / R2, 0…1. */
  triggers: [number, number];
  /** Touchpad fingers, 0…1. */
  fingers: [number, number][];
  gyro: [number, number, number] | null;
  accel: [number, number, number] | null;
}

export const CONTROLLERS_CHANGED = "controllers-changed";
export const CONTROLLER_INPUT = "controller-input";

export const controllersApi = {
  status: () => invoke<ControllersStatus>("controllers_status"),
  live: (on: boolean) => invoke<void>("controllers_live", { on }),
  rumble: (id: number, strong: boolean) => invoke<void>("controller_rumble", { id, strong }),
  motion: (id: number, enabled: boolean) => invoke<void>("controller_motion", { id, enabled }),
};

export interface Collection {
  id: number;
  name: string;
  gameCount: number;
}

export interface UninstallInfo {
  installDir: string | null;
  uninstaller: string | null;
}

/** Emitted at start-up with the number of new games waiting in the watched folders. */
export const NEW_GAMES_FOUND = "new-games-found";

export const manageApi = {
  relocate: (id: number, dir: string) => invoke<void>("relocate_game", { id, dir }),
  uninstallInfo: (id: number) => invoke<UninstallInfo>("uninstall_info", { id }),
  uninstall: (id: number, mode: "uninstaller" | "recycle") =>
    invoke<void>("uninstall_game", { id, mode }),
  setImageFile: (id: number, kind: ArtKind, path: string) =>
    invoke<void>("set_game_image_file", { id, kind, path }),
  addAction: (gameId: number, name: string, target: string, args: string) =>
    invoke<number>("add_action", { gameId, name, target, args }),
  removeAction: (id: number) => invoke<void>("remove_action", { id }),
  setDefaultAction: (id: number) => invoke<void>("set_default_action", { id }),
  collections: () => invoke<Collection[]>("list_collections"),
  createCollection: (name: string) => invoke<number>("create_collection", { name }),
  deleteCollection: (id: number) => invoke<void>("delete_collection", { id }),
  setGameCollections: (id: number, collections: number[]) =>
    invoke<void>("set_game_collections", { id, collections }),
};

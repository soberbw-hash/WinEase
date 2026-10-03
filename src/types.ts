export type SectionId =
  | "home"
  | "health"
  | "network"
  | "applications"
  | "system"
  | "components"
  | "efficiency"
  | "cleaning"
  | "settings";

export type CleaningScan = {
  scanId: string;
  categories: Array<{
    id: string;
    label: string;
    sizeBytes: number;
    fileCount: number;
  }>;
  groups: CleaningGroup[];
  skippedEntries: number;
};
export type WindowsSetting = {
  id: string;
  label: string;
  enabled: boolean;
  canRestore: boolean;
};

export type ActionId =
  | "launch_capture"
  | "open_apps_features"
  | "open_notifications"
  | "open_windows_update";

export type ComponentOperation =
  "install" | "repair" | "uninstall" | "disable" | "update";

export type SystemSnapshot = {
  hostName: string;
  osName: string;
  osVersion: string;
  osBuild: string;
  cpuName: string;
  cpuLoad: number;
  cpuCores: number;
  logicalCores: number;
  memoryTotalMb: number;
  memoryUsedMb: number;
  memoryUsagePercent: number;
  gpuName?: string | null;
  gpuMemoryMb?: number | null;
  networkName?: string | null;
  networkDescription?: string | null;
  networkLinkSpeed?: string | null;
  collectedAt: string;
};

export type ToolActionResult = {
  actionId: string;
  title: string;
  success: boolean;
  summary: string;
  details: string;
  durationMs: number;
  outputPath?: string | null;
  warnings: string[];
};

export type StorageHotspot = {
  id: string;
  label: string;
  path: string;
  source: string;
  sizeBytes: number;
  itemCount: number;
};

export type ComponentStatus = "not-installed" | "installed" | "repairable";

export type ComponentBusyState = {
  componentId: string;
  operation: ComponentOperation | "launch";
  stageLabel: string;
  progress: number;
};

export type ComponentManifest = {
  id: string;
  name: string;
  description: string;
  category: string;
  kind: "built-in" | "winget" | "embedded";
  status: ComponentStatus;
  installed: boolean;
  statusLabel: string;
  summary: string;
  version?: string | null;
  sourceLabel?: string | null;
  sourceUrl?: string | null;
  licenseName?: string | null;
  licenseUrl?: string | null;
  installSize?: string | null;
  wingetId?: string | null;
  homepage?: string | null;
  launchPath?: string | null;
  launchArguments?: string[] | null;
  installDir?: string | null;
  logDir?: string | null;
  supportsRepair: boolean;
  supportsUninstall: boolean;
  supportsUpdate: boolean;
  recommended: boolean;
};

export type ThirdPartyNotice = {
  id: string;
  name: string;
  version: string;
  sourceLabel: string;
  sourceUrl: string;
  licenseName: string;
  licenseUrl?: string | null;
  notes: string;
};

export type SectionConfig = {
  label: string;
  hint: string;
  eyebrow: string;
  title: string;
  description: string;
};

export type ToolDefinition = {
  id: ActionId;
  title: string;
  description: string;
  tag: string;
  note: string;
  tone?: "primary" | "default";
};

export type HomeQuickAction = {
  id: string;
  title: string;
  description: string;
  tone?: "primary" | "default";
};

export type BossModeViewState = {
  stageTitle: string;
  stageHint: string;
  percent: number;
  phaseLabel: string;
  phaseIndex: number;
  isRebooting: boolean;
  instruction: string;
};

export type AppSettings = {
  captureHelperEnabled: boolean;
};

export type CleaningGroup = {
  id: string;
  label: string;
  category: string;
  path: string;
  sizeBytes: number;
  fileCount: number;
  recommended: boolean;
};

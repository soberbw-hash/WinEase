export type ResourceReport = {
  seconds: number;
  groups: Array<{name: string; path: string; cpuPercent: number; memoryBytes: number; processCount: number; attention: boolean}>;
  boot: {durationMs: number; time: string} | null;
  bootError: string | null;
  startup: Array<{name: string | null; path: string | null; durationMs: number | null; delayMs: number | null}>;
};
export type MaintenanceReport = {checks: Array<{id: string; title: string; status: string; detail: string; repair: string | null}>; pendingReboot: boolean; canRestoreServices?: boolean};
export type AppPackage = {iconTarget: string | null; name: string; fullName: string; family: string; version: string; location: string; dependencies: string[]; sizeBytes: number | null; sizeLimited: boolean; label: string; purpose: string; removable: boolean; removed: boolean};

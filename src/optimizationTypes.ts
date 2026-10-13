export type OptimizationItem = {
  id: string; title: string; detail: string; status: string; sizeBytes: number; fileCount: number;
  selectable: boolean; defaultSelected: boolean; actionLabel: string | null; target: string | null;
  iconTarget: string | null; iconKind: "application" | "command" | "file" | "system" | "network";
  hasFiles: boolean;
};
export type OptimizationSection = {
  id: string; title: string; status: "checking" | "complete" | "error"; summary: string; items: OptimizationItem[];
};
export type OptimizationReport = {
  id: string; revision: number; phase: "checking" | "ready" | "optimizing" | "complete" | "expired";
  checkedAt: string; sections: OptimizationSection[];
  outcomes: Array<{itemIds: string[]; title: string; status: "success" | "failed"; message: string}>;
  completedActions: number; totalActions: number;
};
export type HealthReport = {
  checkedAt: string;
  checks: Array<{id: string; title: string; status: string; detail: string; target: string | null}>;
};

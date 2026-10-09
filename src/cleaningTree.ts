import type { CleaningGroup } from "./types";
export type CleaningNode = {
  id: string;
  label: string;
  ids: string[];
  groups: CleaningGroup[];
  sizeBytes: number;
  fileCount: number;
  iconTarget?: string | null;
  children: CleaningNode[];
};
function node(
  id: string,
  label: string,
  groups: CleaningGroup[],
  children: CleaningNode[] = [],
): CleaningNode {
  return {
    id,
    label,
    groups,
    ids: groups.map((g) => g.id),
    sizeBytes: groups.reduce((sum, g) => sum + g.sizeBytes, 0),
    fileCount: groups.reduce((sum, g) => sum + g.fileCount, 0),
    iconTarget: groups.find((g) => g.iconTarget)?.iconTarget,
    children,
  };
}
function cacheKind(group: CleaningGroup) {
  const path = group.path.toLowerCase().replace(/\\$/, "");
  if (path.endsWith("code cache")) return "代码缓存";
  if (
    [
      "gpucache",
      "shadercache",
      "dawngraphitecache",
      "dawnwebgpucache",
      "dxcache",
      "glcache",
      "d3dscache",
    ].some((kind) => path.endsWith(kind))
  )
    return "图形缓存";
  return group.label.split(" · ").slice(-1)[0] ?? group.label;
}
export function buildCleaningTree(groups: CleaningGroup[]): CleaningNode[] {
  const owners = new Map<string, CleaningGroup[]>();
  for (const group of groups) {
    const owner = group.label.includes(" · ")
      ? group.label.split(" · ")[0]
      : "Windows";
    owners.set(owner, [...(owners.get(owner) ?? []), group]);
  }
  return [...owners.entries()]
    .map(([owner, items]) => {
      const kinds = new Map<string, CleaningGroup[]>();
      for (const item of items) {
        const kind = cacheKind(item);
        kinds.set(kind, [...(kinds.get(kind) ?? []), item]);
      }
      const children = [...kinds.entries()]
        .map(([kind, items]) => node(`kind:${owner}:${kind}`, kind, items))
        .sort((a, b) => b.sizeBytes - a.sizeBytes);
      return node(`app:${owner}`, owner, items, children);
    })
    .sort((a, b) => b.sizeBytes - a.sizeBytes);
}

import type { CleaningGroup } from "./types";
export function cleaningDescription(label: string) {
  if (/代码缓存/.test(label)) return "浏览器或软件预编译的代码，清理后会重新生成。";
  if (/网页|浏览器|Internet/.test(label)) return "网站图片和页面的临时副本；不是密码或登录记录。";
  if (/图形|缩略图|着色器/.test(label)) return "图片预览或游戏绘图缓存；重建时可能暂时变慢。";
  if (/日志|报告|转储/.test(label)) return "软件运行与出错记录，删除后会减少历史排错信息。";
  if (/下载缓存|Yarn|npm|包缓存/.test(label)) return "可重新下载的安装或开发缓存；下次使用可能需要联网。";
  if (/临时/.test(label)) return "程序处理任务留下的临时文件，保留近期和正在使用的内容。";
  return "软件运行时生成的缓存，清理后按需重建；个人文件保留。";
}
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

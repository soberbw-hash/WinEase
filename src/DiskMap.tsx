import { useMemo, useState } from "react";
import { formatBytes } from "./format";
export type UsageNode = {
  path: string;
  name: string;
  sizeBytes: number;
  fileCount: number;
};
type Item = { path: string; name: string; sizeBytes: number; folder: boolean };
const normalize = (path: string) =>
  path
    .replace(/^\\\\\?\\/, "")
    .replace(/\\$/, "")
    .toLowerCase();
const parent = (path: string) =>
  normalize(path).substring(0, normalize(path).lastIndexOf("\\"));
export function rectangles(
  items: Item[],
  x = 0,
  y = 0,
  width = 100,
  height = 100,
): (Item & { x: number; y: number; width: number; height: number })[] {
  if (!items.length) return [];
  if (items.length === 1) return [{ ...items[0], x, y, width, height }];
  const total = items.reduce((n, i) => n + i.sizeBytes, 0);
  let split = 1,
    sum = items[0].sizeBytes;
  while (split < items.length - 1 && sum < total / 2)
    sum += items[split++].sizeBytes;
  const ratio = sum / total;
  return width >= height
    ? [
        ...rectangles(items.slice(0, split), x, y, width * ratio, height),
        ...rectangles(
          items.slice(split),
          x + width * ratio,
          y,
          width * (1 - ratio),
          height,
        ),
      ]
    : [
        ...rectangles(items.slice(0, split), x, y, width, height * ratio),
        ...rectangles(
          items.slice(split),
          x,
          y + height * ratio,
          width,
          height * (1 - ratio),
        ),
      ];
}
function directoriesPath(path: string, usage: UsageNode[]) {return usage.some(n => normalize(n.path) === normalize(path));}
export function DiskMap({
  root,
  usage,
  files,
  total,
  onOpen,
}: {
  root: string;
  usage: UsageNode[];
  files: { path: string; name: string; sizeBytes: number }[];
  total: number;
  onOpen: (path: string) => void;
}) {
  const [folder, setFolder] = useState(root);
  const key = normalize(folder);
  const { visible, size } = useMemo(() => {
    const directories = new Map(usage.map(n => [normalize(n.path), n]));
    const byParent = new Map<string, Item[]>();
    for (const n of [...usage.map(n => ({...n, folder: true})), ...files.map(n => ({...n, folder: false}))]) {
      if (!Number.isFinite(n.sizeBytes) || n.sizeBytes <= 0) continue;
      const owner = parent(n.path), list = byParent.get(owner) ?? [];
      if (!list.some(item => normalize(item.path) === normalize(n.path))) list.push(n);
      byParent.set(owner, list);
    }
    function children(path: string, bytes: number): Item[] {
      const list = [...(byParent.get(normalize(path)) ?? [])].sort((a,b) => b.sizeBytes-a.sizeBytes);
      const used = list.reduce((sum,n) => sum+n.sizeBytes,0);
      if (bytes > used) list.push({path, name: `${directories.get(normalize(path))?.name ?? '目录'}内其他文件`, sizeBytes: bytes-used, folder: false});
      return list;
    }
    const size = directories.get(key)?.sizeBytes ?? total;
    let visible = children(folder, size);
    const expanded = new Set<string>();
    // Expand large folders automatically, including single-child chains such as Users/user/Documents.
    for (let step=0;step<180;step++) {
      const next = visible.filter(n => n.folder && !expanded.has(normalize(n.path)))
        .sort((a,b)=>b.sizeBytes-a.sizeBytes)[0];
      if (!next) break;
      expanded.add(normalize(next.path));
      const nested = children(next.path,next.sizeBytes);
      if (!nested.length || visible.length-1+nested.length>240) continue;
      visible.splice(visible.indexOf(next),1,...nested);
      if (visible.length>=100) break;
    }
    visible.sort((a,b)=>b.sizeBytes-a.sizeBytes);
    return {visible,size};
  }, [usage, files, key, folder, total]);
  function color(item: Item) {
    const ext = item.folder ? 'folder' : item.name.split('.').pop()?.toLowerCase();
    if (['safetensors','gguf','onnx','pt','pth'].includes(ext ?? '')) return 0;
    if (['mp4','mkv','avi','mov','webm'].includes(ext ?? '')) return 1;
    if (['zip','rar','7z','iso','gz'].includes(ext ?? '')) return 2;
    if (['jpg','png','jpeg','webp','gif','bmp'].includes(ext ?? '')) return 3;
    if (['exe','dll','msi','pak','ucas'].includes(ext ?? '')) return 4;
    if (['mp3','wav','flac','ogg'].includes(ext ?? '')) return 5;
    let hash=0;for(const ch of item.path || item.name) hash=(hash*31+ch.charCodeAt(0))>>>0;
    return 6+hash%4;
  }
  return (
    <div className="disk-map">
      <div className="section-head">
        <div>
          <h3>空间占用图</h3>
          <small>
            {folder.replace(/^\\\\\?\\/, "")} · {formatBytes(size)}
          </small>
        </div>
        <div className="button-row">
          <button
            className="ghost-button"
            disabled={key === normalize(root)}
            onClick={() =>
              setFolder(
                folder.substring(
                  0,
                  folder.replace(/\\$/, "").lastIndexOf("\\"),
                ) + "\\",
              )
            }
          >
            上一级
          </button>
          <button className="ghost-button" onClick={() => setFolder(root)}>
            磁盘根目录
          </button>
        </div>
      </div>
      <p className="scope-note">面积代表占用大小；模型、视频、压缩包等使用不同颜色。点击文件夹下钻，悬停查看路径。</p>
      <div
        className="disk-map__canvas"
        aria-label="空间占用，方块面积代表文件大小"
      >
        {rectangles(visible).map((item, index) => (
          <button
            key={`${item.path}-${index}`}
            className={`disk-map__tile disk-map__color--${color(item)}${item.width < 7 || item.height < 9 ? " disk-map__tile--tiny" : item.height < 14 ? " disk-map__tile--compact" : ""}`}
            style={{
              left: `${item.x}%`,
              top: `${item.y}%`,
              width: `${item.width}%`,
              height: `${item.height}%`,
            }}
            aria-label={`${item.name} ${formatBytes(item.sizeBytes)}`}
            title={`${item.path || item.name} · ${formatBytes(item.sizeBytes)}${item.folder ? " · 点击查看" : ""}`}
            disabled={!item.path}
            onClick={() =>
              item.folder
                ? setFolder(item.path)
                : onOpen(directoriesPath(item.path, usage) ? item.path : item.path.substring(0, item.path.lastIndexOf("\\")))
            }
          >
            <strong>{item.name}</strong>
            <span>{formatBytes(item.sizeBytes)}</span>
          </button>
        ))}
      </div>
    </div>
  );
}

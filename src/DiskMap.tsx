import { useState } from "react";
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
function rectangles(
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
  const children: Item[] = [
    ...usage
      .filter((n) => parent(n.path) === key)
      .map((n) => ({ ...n, folder: true })),
    ...files
      .filter((n) => parent(n.path) === key)
      .map((n) => ({ ...n, folder: false })),
  ];
  const size = usage.find((n) => normalize(n.path) === key)?.sizeBytes ?? total;
  const remainder = Math.max(
    0,
    size - children.reduce((n, i) => n + i.sizeBytes, 0),
  );
  if (remainder)
    children.push({
      path: "",
      name: "其他文件",
      sizeBytes: remainder,
      folder: false,
    });
  children.sort((a, b) => b.sizeBytes - a.sizeBytes);
  const visible = children.slice(0, 60);
  if (children.length > 60)
    visible.push({
      path: "",
      name: "其他项目",
      sizeBytes: children.slice(60).reduce((n, i) => n + i.sizeBytes, 0),
      folder: false,
    });
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
      <div
        className="disk-map__canvas"
        aria-label="空间占用，方块面积代表文件大小"
      >
        {rectangles(visible).map((item, index) => (
          <button
            key={`${item.path}-${index}`}
            className={`disk-map__tile disk-map__tile--${index % 4}`}
            style={{
              left: `${item.x}%`,
              top: `${item.y}%`,
              width: `${item.width}%`,
              height: `${item.height}%`,
            }}
            title={`${item.name} · ${formatBytes(item.sizeBytes)}${item.folder ? " · 点击查看" : ""}`}
            disabled={!item.path}
            onClick={() =>
              item.folder
                ? setFolder(item.path)
                : onOpen(item.path.substring(0, item.path.lastIndexOf("\\")))
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

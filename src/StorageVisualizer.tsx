import { formatBytes } from "./format";
import type { StorageHotspot } from "./types";

type StorageVisualizerProps = {
  hotspots: StorageHotspot[];
  onOpenTarget: (target: string) => void;
};

const colors = [
  "#4f7cff",
  "#75b7ff",
  "#96c8ff",
  "#b3d4ff",
  "#d5e5ff",
  "#ebe6d7",
  "#f3d6a8",
  "#e9b66c",
];

function buildGradient(hotspots: StorageHotspot[]) {
  const total = hotspots.reduce((sum, item) => sum + item.sizeBytes, 0);
  if (!total) {
    return "conic-gradient(#e8edf7 0deg 360deg)";
  }

  let current = 0;
  const segments = hotspots.map((item, index) => {
    const portion = (item.sizeBytes / total) * 360;
    const start = current;
    current += portion;
    return `${colors[index % colors.length]} ${start}deg ${current}deg`;
  });

  return `conic-gradient(${segments.join(", ")})`;
}

function getSpan(sizeBytes: number, total: number) {
  const ratio = total ? sizeBytes / total : 0;

  if (ratio >= 0.24) {
    return 6;
  }

  if (ratio >= 0.16) {
    return 4;
  }

  if (ratio >= 0.1) {
    return 3;
  }

  return 2;
}

export function StorageVisualizer({
  hotspots,
  onOpenTarget,
}: StorageVisualizerProps) {
  const visible = hotspots.slice(0, 8);
  const total = visible.reduce((sum, item) => sum + item.sizeBytes, 0);

  if (visible.length === 0) {
    return (
      <div className="empty-state">
        还没有扫描到空间热点，点一下刷新再看看。
      </div>
    );
  }

  return (
    <div className="storage-visual">
      <div className="storage-visual__summary soft-card">
        <div
          className="storage-donut"
          style={{ backgroundImage: buildGradient(visible) }}
        >
          <div className="storage-donut__inner">
            <strong>{formatBytes(total)}</strong>
            <span>已扫描热点</span>
          </div>
        </div>

        <div className="storage-legend">
          {visible.slice(0, 5).map((item, index) => (
            <button
              key={item.id}
              className="storage-legend__item"
              type="button"
              onClick={() => onOpenTarget(item.path)}
            >
              <span
                className="storage-legend__dot"
                style={{ backgroundColor: colors[index % colors.length] }}
              />
              <strong>{item.label}</strong>
              <small>{formatBytes(item.sizeBytes)}</small>
            </button>
          ))}
        </div>
      </div>

      <div className="storage-map soft-card">
        {visible.map((item, index) => (
          <button
            key={item.id}
            className="storage-map__tile"
            type="button"
            style={{
              backgroundColor: colors[index % colors.length],
              gridColumn: `span ${getSpan(item.sizeBytes, total)}`,
            }}
            onClick={() => onOpenTarget(item.path)}
          >
            <strong>{item.label}</strong>
            <span>{formatBytes(item.sizeBytes)}</span>
            <small>{item.source}</small>
          </button>
        ))}
      </div>

      <div className="storage-list">
        {visible.map((item, index) => (
          <article key={item.id} className="soft-card storage-list__item">
            <div className="history-item__top">
              <div className="storage-list__title">
                <span
                  className="storage-legend__dot"
                  style={{ backgroundColor: colors[index % colors.length] }}
                />
                <strong>{item.label}</strong>
              </div>
              <span className="pill pill--muted">
                {formatBytes(item.sizeBytes)}
              </span>
            </div>
            <p>{item.path}</p>
            <small>{item.itemCount} 项</small>
            <button
              className="ghost-button"
              type="button"
              onClick={() => onOpenTarget(item.path)}
            >
              打开位置
            </button>
          </article>
        ))}
      </div>
    </div>
  );
}

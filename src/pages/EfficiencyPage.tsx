import { quickPathTargets } from "../content";
import { StorageVisualizer } from "../StorageVisualizer";
import type { StorageHotspot } from "../types";

type EfficiencyPageProps = {
  hotspots: StorageHotspot[];
  busy: boolean;
  error: string;
  onRefreshHotspots: () => void;
  onOpenTarget: (target: string) => void;
};

const quickPathMap: Record<
  (typeof quickPathTargets)[number]["pathKey"],
  string
> = {
  downloads: "shell:Downloads",
  desktop: "shell:Desktop",
  documents: "shell:Personal",
};

export function EfficiencyPage({
  hotspots,
  busy,
  error,
  onRefreshHotspots,
  onOpenTarget,
}: EfficiencyPageProps) {
  return (
    <div className="page-stack">
      <section className="surface">
        <div className="section-head">
          <div>
            <h2>空间管理</h2>
          </div>
          <button
            className="ghost-button"
            type="button"
            disabled={busy}
            onClick={onRefreshHotspots}
          >
            {busy ? "扫描中…" : hotspots.length ? "重新扫描" : "开始扫描"}
          </button>
        </div>

        <p className="scope-note">范围：下载、桌面、文档、图片、视频</p>
        {error && (
          <p className="inline-error" role="alert">
            {error}
          </p>
        )}
        {hotspots.length > 0 ? (
          <StorageVisualizer hotspots={hotspots} onOpenTarget={onOpenTarget} />
        ) : (
          <div className="empty-state">
            {busy ? "正在扫描" : "点击开始扫描"}
          </div>
        )}
      </section>

      <section className="surface">
        <div className="section-head">
          <div>
            <h2>常用目录</h2>
          </div>
        </div>

        <div className="quick-path-grid">
          {quickPathTargets.map((item) => (
            <button
              key={item.id}
              className="quick-path"
              type="button"
              onClick={() => onOpenTarget(quickPathMap[item.pathKey])}
            >
              <strong>{item.label}</strong>
              <span>快速打开</span>
            </button>
          ))}
        </div>
      </section>
    </div>
  );
}

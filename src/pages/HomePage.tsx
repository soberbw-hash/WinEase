import type { HomeQuickAction } from "../types";
export function HomePage({
  quickActions,
  onQuickAction,
}: {
  quickActions: HomeQuickAction[];
  onQuickAction: (id: string) => void;
}) {
  return (
    <div className="page-stack home-page">
      <section className="surface">
        <div className="section-head">
          <h2>主要功能</h2>
        </div>
        <div className="home-actions">
          {quickActions.map((item) => (
            <button
              key={item.id}
              className={`home-action ${item.tone === "primary" ? "home-action--primary" : ""}`}
              onClick={() => onQuickAction(item.id)}
            >
              <strong>{item.title}</strong>
              <span>{item.description}</span>
              <span className="home-action__arrow" aria-hidden="true">
                →
              </span>
            </button>
          ))}
        </div>
      </section>
      <section className="surface">
        <div className="section-head">
          <h2>常用工具</h2>
        </div>
        <div className="home-shortcuts">
          {[
            ["open_processes", "进程管理"],
            ["open_duplicates", "重复文件"],
            ["open_network", "网络检测"],
            ["open_system", "Windows 设置"],
            ["open_uninstall", "深度卸载"],
            ["open_popups", "弹窗管理"],
          ].map(([id, label]) => (
            <button
              className="quick-path"
              key={id}
              onClick={() => onQuickAction(id)}
            >
              <strong>{label}</strong>
              <span aria-hidden="true">→</span>
            </button>
          ))}
        </div>
      </section>
    </div>
  );
}

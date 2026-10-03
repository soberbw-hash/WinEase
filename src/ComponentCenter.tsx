import { getComponentIconPath } from "./componentIcons";
import type {
  ComponentBusyState,
  ComponentManifest,
  ComponentOperation,
} from "./types";

type ComponentCenterProps = {
  items: ComponentManifest[];
  busyState: Record<string, ComponentBusyState>;
  hiddenIds?: string[];
  captureHelperEnabled?: boolean;
  onToggleCaptureHelper?: (enabled: boolean) => void;
  onManage: (componentId: string, operation: ComponentOperation) => void;
  onLaunch: (componentId: string) => void;
  onOpenTarget: (target: string) => void;
};

function getCategoryTone(category: string) {
  if (category.includes("网络")) {
    return "component-card__icon--network";
  }

  if (category.includes("系统")) {
    return "component-card__icon--system";
  }

  return "component-card__icon--efficiency";
}

function getPrimaryAction(item: ComponentManifest) {
  if (item.status === "repairable") {
    return { label: "修复", operation: "repair" as const };
  }

  if (item.installed) {
    return { label: "打开", operation: null };
  }

  return { label: "安装", operation: "install" as const };
}

export function ComponentCenter({
  items,
  busyState,
  hiddenIds = [],
  captureHelperEnabled = false,
  onToggleCaptureHelper,
  onManage,
  onLaunch,
}: ComponentCenterProps) {
  const visibleItems = items.filter(
    (item) => item.kind !== "built-in" && !hiddenIds.includes(item.id),
  );

  return (
    <section className="surface">
      <div className="section-head">
        <div>
          <h2>组件中心</h2>
        </div>
      </div>

      {visibleItems.length === 0 ? (
        <div className="empty-state">当前还没有可展示的组件。</div>
      ) : (
        <div className="component-grid">
          {visibleItems.map((item) => {
            const task = busyState[item.id];
            const isBusy = Boolean(task);
            const primary = getPrimaryAction(item);
            const iconPath = getComponentIconPath(item.id);

            return (
              <article
                key={item.id}
                className="soft-card component-card component-card--rich"
              >
                <div className="component-card__top">
                  <div className="component-card__title">
                    <span
                      className={`component-card__icon ${getCategoryTone(item.category)}`}
                    >
                      {iconPath ? (
                        <img src={iconPath} alt="" />
                      ) : (
                        item.name.slice(0, 1)
                      )}
                    </span>
                    <div>
                      <h3 title={item.name}>{item.name}</h3>
                      <small>{item.category}</small>
                    </div>
                  </div>
                  <span
                    className={`pill ${
                      item.status === "repairable"
                        ? "pill--warning"
                        : item.installed
                          ? "pill--success"
                          : "pill--muted"
                    }`}
                  >
                    {item.statusLabel}
                  </span>
                </div>

                <p title={item.description}>{item.description}</p>

                {isBusy ? (
                  <div className="component-progress">
                    <div className="component-progress__head">
                      <span>{task?.stageLabel}</span>
                    </div>
                  </div>
                ) : null}

                <div className="button-row">
                  <button
                    className={
                      item.installed ? "secondary-button" : "primary-button"
                    }
                    type="button"
                    disabled={isBusy}
                    onClick={() => {
                      if (primary.operation) {
                        onManage(item.id, primary.operation);
                      } else {
                        onLaunch(item.id);
                      }
                    }}
                  >
                    {isBusy ? "处理中..." : primary.label}
                  </button>

                  {item.installed &&
                  item.supportsRepair &&
                  item.status !== "repairable" ? (
                    <button
                      className="ghost-button"
                      type="button"
                      disabled={isBusy}
                      onClick={() => onManage(item.id, "repair")}
                    >
                      修复
                    </button>
                  ) : null}

                  {item.installed && item.supportsUpdate && (
                    <button
                      className={
                        item.updateAvailable
                          ? "secondary-button"
                          : "ghost-button"
                      }
                      disabled={isBusy}
                      onClick={() => onManage(item.id, "update")}
                      title={
                        item.availableVersion
                          ? `可更新至 ${item.availableVersion}`
                          : "检查并更新"
                      }
                    >
                      更新{item.updateAvailable ? " · 新版" : ""}
                    </button>
                  )}
                  {item.supportsUninstall && item.installed ? (
                    <button
                      className="ghost-button"
                      type="button"
                      disabled={isBusy}
                      onClick={() => onManage(item.id, "uninstall")}
                    >
                      卸载
                    </button>
                  ) : null}
                  {item.id === "capture-plus" &&
                    item.installed &&
                    onToggleCaptureHelper && (
                      <label
                        className="component-capture-toggle"
                        title="F1 截图 · F3 贴图"
                      >
                        <input
                          type="checkbox"
                          checked={captureHelperEnabled}
                          disabled={isBusy}
                          onChange={(event) =>
                            onToggleCaptureHelper(event.target.checked)
                          }
                        />
                        快捷键
                      </label>
                    )}
                </div>
              </article>
            );
          })}
        </div>
      )}
    </section>
  );
}

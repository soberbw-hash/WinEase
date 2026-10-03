import { useState } from "react";
import { bossModeShortcut } from "../content";
import { version } from "../../package.json";
import type {
  AppSettings,
  ComponentBusyState,
  ComponentManifest,
  ComponentOperation,
  ThirdPartyNotice,
  SystemSnapshot,
} from "../types";

type Props = {
  snapshot: SystemSnapshot | null;
  onOpenDeviceInfo: () => void;
  settings: AppSettings;
  components: ComponentManifest[];
  notices: ThirdPartyNotice[];
  busyState: ComponentBusyState | null;
  onUpdateSettings: (patch: Partial<AppSettings>) => void;
  onEnterBossMode: () => void;
  onManageComponent: (id: string, operation: ComponentOperation) => void;
  onLaunchComponent: (id: string) => void;
  onOpenTarget: (target: string) => void;
  onOpenSupportModal: () => void;
};
export function SettingsPage({
  snapshot,
  onOpenDeviceInfo,
  settings,
  components,
  notices,
  busyState,
  onUpdateSettings,
  onEnterBossMode,
  onManageComponent,
  onLaunchComponent,
  onOpenTarget,
  onOpenSupportModal,
}: Props) {
  const [section, setSection] = useState("general");
  const installed = components.filter(
    (item) => item.installed && item.kind !== "built-in",
  );
  return (
    <div className="page-stack">
      <nav className="subnav" aria-label="设置分类">
        {[
          { id: "general", label: "通用" },
          { id: "components", label: "组件" },
          { id: "about", label: "关于" },
        ].map((item) => (
          <button
            type="button"
            key={item.id}
            aria-pressed={section === item.id}
            onClick={() => setSection(item.id)}
          >
            {item.label}
          </button>
        ))}
      </nav>
      {section === "general" && (
        <>
          <section className="surface">
            <div className="section-head">
              <h2>外观</h2>
            </div>
            <div className="settings-grid">
              <label className="setting-row soft-card">
                <span>字体</span>
                <select
                  value={settings.fontPreset}
                  onChange={(event) =>
                    onUpdateSettings({
                      fontPreset: event.target
                        .value as AppSettings["fontPreset"],
                    })
                  }
                >
                  <option value="system">系统字体</option>
                  <option value="harmony">鸿蒙字体（需已安装）</option>
                </select>
              </label>
              <label className="setting-row soft-card">
                <span>界面密度</span>
                <select
                  value={settings.density}
                  onChange={(event) =>
                    onUpdateSettings({
                      density: event.target.value as AppSettings["density"],
                    })
                  }
                >
                  <option value="auto">自动</option>
                  <option value="compact">紧凑</option>
                  <option value="standard">标准</option>
                  <option value="comfortable">宽松</option>
                </select>
              </label>
              <label className="setting-row soft-card">
                <span>文字大小</span>
                <select
                  value={settings.scale}
                  onChange={(event) =>
                    onUpdateSettings({
                      scale: event.target.value as AppSettings["scale"],
                    })
                  }
                >
                  <option value="auto">自动</option>
                  <option value="compact">小</option>
                  <option value="standard">标准</option>
                  <option value="relaxed">大</option>
                </select>
              </label>
            </div>
          </section>
          <section className="surface">
            <div className="section-head">
              <h2>设备信息</h2>
              <button className="ghost-button" onClick={onOpenDeviceInfo}>
                详情与日志
              </button>
            </div>
            <div className="setting-line">
              <span>系统</span>
              <strong className="single-line" title={snapshot?.osName}>
                {snapshot?.osName.replace(/^Microsoft\s+/i, "") ?? "读取中"}
              </strong>
            </div>
            <div className="setting-line">
              <span>设备</span>
              <strong className="single-line">
                {snapshot?.hostName ?? "—"}
              </strong>
            </div>
          </section>
          <section className="surface">
            <div className="section-head">
              <h2>快捷键</h2>
            </div>
            <div className="setting-line">
              <div>
                <strong>老板键</strong>
                <p className="scope-note">{bossModeShortcut} · Esc 退出</p>
              </div>
              <button
                className="secondary-button"
                type="button"
                onClick={onEnterBossMode}
              >
                进入
              </button>
            </div>
          </section>
        </>
      )}
      {section === "components" && (
        <>
          <section className="surface">
            <div className="section-head">
              <h2>已安装组件</h2>
            </div>
            {installed.length === 0 ? (
              <div className="empty-state">暂无已安装组件</div>
            ) : (
              <div className="settings-installed-grid">
                {installed.map((item) => (
                  <article
                    key={item.id}
                    className="soft-card installed-component-card"
                  >
                    <div className="history-item__top">
                      <h3>{item.name}</h3>
                      <span className="pill pill--success">
                        {item.statusLabel}
                      </span>
                    </div>
                    {busyState?.componentId === item.id && (
                      <p role="status">{busyState.stageLabel}…</p>
                    )}
                    <div className="button-row">
                      <button
                        className="secondary-button"
                        type="button"
                        disabled={busyState !== null}
                        onClick={() => onLaunchComponent(item.id)}
                      >
                        打开
                      </button>
                      {item.supportsRepair && (
                        <button
                          className="ghost-button"
                          type="button"
                          disabled={busyState !== null}
                          onClick={() => onManageComponent(item.id, "repair")}
                        >
                          修复
                        </button>
                      )}
                      {item.supportsUninstall && (
                        <button
                          className="ghost-button"
                          type="button"
                          disabled={busyState !== null}
                          onClick={() =>
                            onManageComponent(item.id, "uninstall")
                          }
                        >
                          卸载
                        </button>
                      )}
                      {item.logDir && (
                        <button
                          className="ghost-button"
                          type="button"
                          onClick={() => onOpenTarget(item.logDir!)}
                        >
                          日志
                        </button>
                      )}
                    </div>
                  </article>
                ))}
              </div>
            )}
          </section>
          <section className="surface">
            <div className="section-head">
              <h2>第三方许可证</h2>
            </div>
            <div className="notice-grid">
              {notices.map((notice) => (
                <article className="soft-card notice-card" key={notice.id}>
                  <div className="history-item__top">
                    <h3>{notice.name}</h3>
                    <small>{notice.licenseName}</small>
                  </div>
                  <div className="button-row">
                    <button
                      className="ghost-button"
                      onClick={() => onOpenTarget(notice.sourceUrl)}
                    >
                      来源
                    </button>
                    {notice.licenseUrl && (
                      <button
                        className="ghost-button"
                        onClick={() => onOpenTarget(notice.licenseUrl!)}
                      >
                        许可证
                      </button>
                    )}
                  </div>
                </article>
              ))}
            </div>
          </section>
        </>
      )}
      {section === "about" && (
        <section className="surface about-panel">
          <img src="/brand-icon.png" alt="" width="64" height="64" />
          <h2>WinEase</h2>
          <p className="scope-note">Windows 工具箱 · {version}</p>
          <div className="about-links">
            <button
              className="setting-line"
              type="button"
              onClick={() => onOpenTarget("https://github.com/soberbw-hash")}
            >
              <span>我的主页</span>
              <span aria-hidden="true">↗</span>
            </button>
            <button
              className="setting-line"
              type="button"
              onClick={() =>
                onOpenTarget("https://github.com/soberbw-hash/WinEase")
              }
            >
              <span>项目仓库</span>
              <span aria-hidden="true">↗</span>
            </button>
            <button
              className="setting-line"
              type="button"
              onClick={() =>
                onOpenTarget(
                  "https://github.com/soberbw-hash/WinEase/issues",
                )
              }
            >
              <span>反馈问题</span>
              <span aria-hidden="true">↗</span>
            </button>
            <button
              className="setting-line"
              type="button"
              onClick={onOpenSupportModal}
            >
              <span>赞助</span>
              <span aria-hidden="true">→</span>
            </button>
          </div>
        </section>
      )}
    </div>
  );
}

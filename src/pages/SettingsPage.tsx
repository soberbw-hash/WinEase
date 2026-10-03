import { useRef, useState } from "react";
import { bossModeShortcut } from "../content";
import { version } from "../../package.json";
import { getComponentIconPath } from "../componentIcons";
import * as Dialog from "@radix-ui/react-dialog";
import fontLicense from "../assets/harmonyos-license.txt?raw";
import type {
  ComponentBusyState,
  ComponentManifest,
  ComponentOperation,
  ThirdPartyNotice,
  SystemSnapshot,
} from "../types";

type Props = {
  snapshot: SystemSnapshot | null;
  onOpenDeviceInfo: () => void;
  components: ComponentManifest[];
  notices: ThirdPartyNotice[];
  busyState: ComponentBusyState | null;
  onEnterBossMode: () => void;
  onManageComponent: (id: string, operation: ComponentOperation) => void;
  onLaunchComponent: (id: string) => void;
  onOpenTarget: (target: string) => void;
  onOpenSupportModal: () => void;
};
export function SettingsPage({
  snapshot,
  onOpenDeviceInfo,
  components,
  notices,
  busyState,
  onEnterBossMode,
  onManageComponent,
  onLaunchComponent,
  onOpenTarget,
  onOpenSupportModal,
}: Props) {
  const [section, setSection] = useState("general");
  const [fontLicenseOpen, setFontLicenseOpen] = useState(false);
  const licenseOrigin = useRef<HTMLElement | null>(null);
  const installed = components.filter(
    (item) => item.installed && item.kind !== "built-in",
  );
  const allNotices: ThirdPartyNotice[] = [
    {
      id: "harmonyos-sans",
      name: "HarmonyOS Sans SC",
      version: "内置",
      sourceLabel: "华为",
      sourceUrl: "https://developer.huawei.com/consumer/cn/design/resource/",
      licenseName: "HarmonyOS Sans 字体许可",
      notes: "",
    },
    ...notices,
  ];
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
                      <div className="settings-component-name">
                        {getComponentIconPath(item.id) && (
                          <img src={getComponentIconPath(item.id)!} alt="" />
                        )}
                        <h3>{item.name}</h3>
                      </div>
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
              {allNotices.map((notice) => (
                <article className="soft-card notice-card" key={notice.id}>
                  <div className="history-item__top">
                    <div className="settings-component-name">
                      {getComponentIconPath(notice.id) && (
                        <img src={getComponentIconPath(notice.id)!} alt="" />
                      )}
                      <h3>{notice.name}</h3>
                    </div>
                    <small>{notice.licenseName}</small>
                  </div>
                  <div className="button-row">
                    <button
                      className="ghost-button"
                      onClick={() => onOpenTarget(notice.sourceUrl)}
                    >
                      来源
                    </button>
                    {(notice.licenseUrl || notice.id === "harmonyos-sans") && (
                      <button
                        className="ghost-button"
                        onClick={() =>
                          notice.id === "harmonyos-sans"
                            ? setFontLicenseOpen(true)
                            : onOpenTarget(notice.licenseUrl!)
                        }
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
          <button
            className="secondary-button"
            onClick={() =>
              window.dispatchEvent(new Event("winease-check-update"))
            }
          >
            检查更新
          </button>
          <p className="scope-note">界面使用 HarmonyOS Sans SC 鸿蒙字体</p>
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
                onOpenTarget("https://github.com/soberbw-hash/WinEase/issues")
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
      <Dialog.Root open={fontLicenseOpen} onOpenChange={setFontLicenseOpen}>
        <Dialog.Portal>
          <Dialog.Overlay className="dialog-overlay" />
          <Dialog.Content
            className="font-license-dialog"
            onOpenAutoFocus={() => {
              licenseOrigin.current = document.activeElement as HTMLElement;
            }}
            onCloseAutoFocus={(event) => {
              event.preventDefault();
              licenseOrigin.current?.focus();
            }}
          >
            <Dialog.Title>鸿蒙字体许可证</Dialog.Title>
            <Dialog.Description className="scope-note">
              HarmonyOS Sans SC · Huawei Device Co., Ltd.
            </Dialog.Description>
            <pre>{fontLicense}</pre>
            <Dialog.Close asChild>
              <button className="secondary-button">关闭</button>
            </Dialog.Close>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
    </div>
  );
}

import { useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { ConfirmDialog } from "../ConfirmDialog";
import { formatBytes } from "../format";
import type { ComponentManifest } from "../types";

type Process = {
  pid: number;
  name: string;
  memoryBytes: number;
  path: string;
  stamp: string;
  title: string;
  canEnd: boolean;
  windowId?: string;
};
type Startup = {
  name: string;
  command: string;
  enabled: boolean;
  editable: boolean;
  source: string;
};
export type ManagementTab = "processes" | "startup" | "uninstall" | "popups";
export function ManagementPage({
  initialTab,
  components,
  componentBusy,
  onComponent,
  onOpenTarget,
}: {
  initialTab: ManagementTab;
  components: ComponentManifest[];
  componentBusy: boolean;
  onComponent: (id: string, installed: boolean) => void;
  onOpenTarget: (target: string) => void;
}) {
  const [tab, setTab] = useState(initialTab);
  const [processes, setProcesses] = useState<Process[]>([]),
    [startup, setStartup] = useState<Startup[]>([]);
  const [busy, setBusy] = useState(false),
    [error, setError] = useState(""),
    [message, setMessage] = useState("");
  const [search, setSearch] = useState("");
  const [pending, setPending] = useState<Process | Startup | null>(null);
  const [blocking, setBlocking] = useState(false);
  const [rules, setRules] = useState<{
    rules: Array<{ id: string; title: string; path: string }>;
    requests: number;
  }>({ rules: [], requests: 0 });
  const lock = useRef(false),
    generation = useRef(0);
  async function refresh(next = tab) {
    const request = ++generation.current;
    setBusy(true);
    setError("");
    try {
      if (next === "startup") {
        const items = await invoke<Startup[]>("list_startup_items");
        if (request === generation.current) setStartup(items);
      } else if (next !== "uninstall") {
        const items = await invoke<Process[]>(
          next === "popups" ? "list_application_windows" : "list_processes",
        );
        if (request === generation.current) setProcesses(items);
        if (next === "popups") {
          const nextRules = await invoke<typeof rules>("list_popup_rules");
          if (request === generation.current) setRules(nextRules);
        }
      }
    } catch (err) {
      if (request === generation.current) setError(String(err));
    } finally {
      if (request === generation.current) setBusy(false);
    }
  }
  useEffect(() => {
    setTab(initialTab);
  }, [initialTab]);
  useEffect(() => {
    setSearch("");
    setMessage("");
    if (isTauri()) void refresh(tab);
    return () => {
      generation.current++;
    };
  }, [tab]);
  useEffect(() => {
    if (tab !== "popups" || !isTauri()) return;
    let active = true;
    const timer = setInterval(() => {
      void invoke<typeof rules>("list_popup_rules")
        .then((next) => {
          if (active) setRules(next);
        })
        .catch(() => {});
    }, 10_000);
    return () => {
      active = false;
      clearInterval(timer);
    };
  }, [tab]);
  async function apply() {
    const item = pending;
    setPending(null);
    if (!item || lock.current) return;
    lock.current = true;
    setBusy(true);
    setError("");
    setMessage("");
    try {
      if ("pid" in item) {
        await invoke(blocking ? "add_popup_rule" : "end_process", {
          pid: item.pid,
          stamp: item.stamp,
          closeWindow: tab === "popups",
          windowId: item.windowId,
        });
        setMessage(
          blocking
            ? "已添加规则，工具箱运行时会关闭此应用的同名窗口。"
            : tab === "popups"
              ? "已发送正常关闭请求。"
              : "进程已结束。",
        );
      } else {
        await invoke("set_startup_item", {
          name: item.name,
          command: item.command,
          enabled: !item.enabled,
        });
        setMessage(item.enabled ? "已停用，可随时恢复。" : "已恢复启动项。");
      }
      await refresh();
    } catch (err) {
      setError(String(err));
    } finally {
      lock.current = false;
      setBusy(false);
    }
  }
  const uninstaller = components.find((c) => c.id === "uninstall-plus");
  async function removeRule(id: string) {
    if (lock.current) return;
    lock.current = true;
    setBusy(true);
    try {
      await invoke("remove_popup_rule", { id });
      setRules(await invoke<typeof rules>("list_popup_rules"));
    } catch (e) {
      setError(String(e));
    } finally {
      lock.current = false;
      setBusy(false);
    }
  }
  const visible = processes.filter(
    (p) =>
      (tab !== "popups" || p.title) &&
      `${p.name} ${p.title}`.toLowerCase().includes(search.toLowerCase()),
  );
  return (
    <div className="page-stack">
      <nav className="subnav" aria-label="应用管理分类">
        {(
          [
            ["processes", "进程管理"],
            ["startup", "开机管理"],
            ["uninstall", "深度卸载"],
            ["popups", "弹窗管理"],
          ] as const
        ).map(([id, label]) => (
          <button
            key={id}
            aria-pressed={tab === id}
            disabled={lock.current}
            onClick={() => setTab(id)}
          >
            {label}
          </button>
        ))}
      </nav>
      <section className="surface">
        <div className="section-head">
          <h2>
            {tab === "startup"
              ? "开机启动项"
              : tab === "popups"
                ? "应用窗口"
                : tab === "uninstall"
                  ? "深度卸载"
                  : "运行中的进程"}
          </h2>
          {tab !== "uninstall" && (
            <button
              className="ghost-button"
              disabled={busy}
              onClick={() => void refresh()}
            >
              刷新
            </button>
          )}
        </div>
        {error && (
          <p className="inline-error" role="alert">
            {error}
          </p>
        )}
        {message && (
          <p className="scope-note" role="status">
            {message}
          </p>
        )}
        {tab === "uninstall" ? (
          <>
            <div className="setting-line">
              <div>
                <h3>BCUninstaller</h3>
                <p className="scope-note">卸载应用并扫描残留文件、注册表项</p>
              </div>
              <button
                className="primary-button"
                disabled={componentBusy || !uninstaller}
                onClick={() =>
                  onComponent("uninstall-plus", Boolean(uninstaller?.installed))
                }
              >
                {componentBusy
                  ? "处理中…"
                  : uninstaller?.installed
                    ? "打开深度卸载"
                    : "安装深度卸载组件"}
              </button>
            </div>
            <button
              className="quick-path"
              onClick={() => onOpenTarget("ms-settings:appsfeatures")}
            >
              <strong>Windows 应用管理</strong>
              <span>查看已安装应用 ↗</span>
            </button>
          </>
        ) : tab === "startup" ? (
          <>
            <p className="scope-note">
              当前用户启动项可停用和恢复；其他启动来源在 Windows 中管理。
            </p>
            <button
              className="secondary-button"
              onClick={() => onOpenTarget("ms-settings:startupapps")}
            >
              全部启动应用 ↗
            </button>
            <div className="manager-list">
              {startup.map((p, i) => (
                <div className="manager-row" key={`${p.source}-${p.name}-${i}`}>
                  <div className="manager-row__text">
                    <strong>{p.name}</strong>
                    <small title={p.command}>
                      {p.source} · {p.command}
                    </small>
                  </div>
                  <button
                    className="secondary-button"
                    disabled={busy || !p.editable}
                    onClick={() => {
                      setBlocking(false);
                      setPending(p);
                    }}
                  >
                    {p.editable
                      ? p.enabled
                        ? "停用"
                        : "恢复"
                      : "Windows 管理"}
                  </button>
                </div>
              ))}
            </div>
          </>
        ) : (
          <>
            {tab === "popups" && (
              <div className="setting-line">
                <p className="scope-note">
                  选择弹窗关闭或拦截同名窗口；通知可按应用关闭。
                </p>
                <button
                  className="secondary-button"
                  onClick={() => onOpenTarget("ms-settings:notifications")}
                >
                  通知设置 ↗
                </button>
              </div>
            )}
            <label className="search-label">
              <span className="sr-only">搜索进程</span>
              <input
                type="search"
                placeholder="搜索应用"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
              />
            </label>
            <div className="manager-list">
              {visible.map((p) => (
                <div className="manager-row" key={p.windowId ?? p.pid}>
                  <div className="manager-row__text">
                    <strong>{tab === "popups" ? p.title : p.name}</strong>
                    <small title={p.path}>
                      {tab === "popups" ? p.name : `PID ${p.pid}`} · {p.path}
                    </small>
                  </div>
                  <span className="tabular">{formatBytes(p.memoryBytes)}</span>
                  {tab === "popups" && p.canEnd && (
                    <button
                      className="ghost-button"
                      disabled={busy}
                      onClick={() => {
                        setBlocking(true);
                        setPending(p);
                      }}
                    >
                      拦截同类
                    </button>
                  )}
                  <button
                    className="secondary-button"
                    disabled={busy || !p.canEnd}
                    onClick={() => {
                      setBlocking(false);
                      setPending(p);
                    }}
                  >
                    {!p.canEnd
                      ? "受保护"
                      : tab === "popups"
                        ? "关闭窗口"
                        : "结束"}
                  </button>
                </div>
              ))}
            </div>
          </>
        )}
        {tab !== "uninstall" && busy && (
          <p role="status" className="scope-note">
            读取中…
          </p>
        )}
        {!busy &&
          tab !== "uninstall" &&
          (tab === "startup" ? startup.length === 0 : visible.length === 0) && (
            <div className="empty-state">暂无项目</div>
          )}
      </section>
      {tab === "popups" && (
        <section className="surface">
          <div className="section-head">
            <h2>拦截规则</h2>
            <small>本次已发送 {rules.requests} 次关闭请求</small>
          </div>
          <p className="scope-note">
            工具箱运行时生效，按应用路径和完整窗口标题匹配。
          </p>
          {rules.rules.map((rule) => (
            <div className="manager-row" key={rule.id}>
              <div className="manager-row__text">
                <strong>{rule.title}</strong>
                <small title={rule.path}>{rule.path}</small>
              </div>
              <button
                className="ghost-button"
                disabled={busy}
                onClick={() => void removeRule(rule.id)}
              >
                取消拦截
              </button>
            </div>
          ))}
          {rules.rules.length === 0 && (
            <div className="empty-state">暂无拦截规则</div>
          )}
        </section>
      )}
      <ConfirmDialog
        open={pending !== null}
        title={
          pending && "pid" in pending
            ? `${blocking ? "拦截同类弹窗" : tab === "popups" ? "关闭窗口" : "结束进程"}？`
            : pending && pending.enabled
              ? "停用启动项？"
              : "恢复启动项？"
        }
        description={
          pending && "pid" in pending
            ? blocking
              ? `${pending.name}：工具箱运行时，将自动关闭标题为“${pending.title}”的窗口。可在规则列表取消。`
              : `${pending.name}：未保存的内容可能丢失。`
            : pending
              ? `${pending.name}：${pending.enabled ? "下次登录时不再自动启动，可恢复。" : "下次登录时自动启动。"}`
              : ""
        }
        confirmLabel="确认"
        onCancel={() => setPending(null)}
        onConfirm={() => void apply()}
      />
    </div>
  );
}

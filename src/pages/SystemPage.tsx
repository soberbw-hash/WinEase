import { useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { windowsSettingLinks } from "../content";
import type { ToolDefinition, WindowsSetting } from "../types";
import { ConfirmDialog } from "../ConfirmDialog";

export function SystemPage({
  tools,
  runningActionId,
  onRunAction,
  onOpenTarget,
}: {
  tools: ToolDefinition[];
  runningActionId: string | null;
  onRunAction: (id: ToolDefinition["id"]) => void;
  onOpenTarget: (target: string) => void;
}) {
  const [settings, setSettings] = useState<WindowsSetting[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const lock = useRef(false);
  const [power, setPower] = useState<{
    mode: string;
    performanceAvailable: boolean;
  } | null>(null);
  const [repairConfirm, setRepairConfirm] = useState(false);
  async function refresh() {
    const [items, plan] = await Promise.all([
      invoke<WindowsSetting[]>("get_windows_settings"),
      invoke<{ mode: string; performanceAvailable: boolean }>("get_power_plan"),
    ]);
    setSettings(items);
    setPower(plan);
  }
  async function setPlan(mode: string) {
    if (lock.current) return;
    lock.current = true;
    setBusy(true);
    setError("");
    try {
      await invoke("set_power_plan", { mode });
      await refresh();
      setMessage("已切换电源计划。");
    } catch (e) {
      setError(String(e));
    } finally {
      lock.current = false;
      setBusy(false);
    }
  }
  async function repair() {
    setRepairConfirm(false);
    if (lock.current) return;
    lock.current = true;
    setBusy(true);
    setError("");
    try {
      await invoke("repair_taskbar");
      setMessage("已重启当前会话的资源管理器。");
    } catch (e) {
      setError(String(e));
    } finally {
      lock.current = false;
      setBusy(false);
    }
  }
  useEffect(() => {
    if (isTauri()) void refresh().catch((err) => setError(String(err)));
  }, []);
  async function change(id: string, enabled?: boolean) {
    if (lock.current) return;
    lock.current = true;
    setBusy(true);
    setError("");
    setMessage("");
    try {
      await invoke(
        enabled === undefined
          ? "restore_windows_setting"
          : "set_windows_setting",
        { id, enabled },
      );
      await refresh();
      setMessage("已保存，重新打开资源管理器窗口后生效。");
    } catch (err) {
      setError(String(err));
    } finally {
      lock.current = false;
      setBusy(false);
    }
  }
  return (
    <div className="page-stack">
      <section className="surface">
        <div className="section-head">
          <h2>Windows 设置</h2>
          <button
            className="ghost-button"
            disabled={busy}
            onClick={() => void refresh().catch((err) => setError(String(err)))}
          >
            刷新状态
          </button>
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
        {settings.length === 0 ? (
          <div className="empty-state">
            {isTauri() ? "读取设置中" : "桌面应用中可修改设置"}
          </div>
        ) : (
          settings.map((item) => (
            <div className="setting-line" key={item.id}>
              <label>
                <input
                  type="checkbox"
                  checked={item.enabled}
                  disabled={busy}
                  onChange={(event) =>
                    void change(item.id, event.target.checked)
                  }
                />
                {item.label}
              </label>
              <button
                className="ghost-button"
                type="button"
                disabled={busy || !item.canRestore}
                onClick={() => void change(item.id)}
              >
                恢复原设置
              </button>
            </div>
          ))
        )}
        <div className="setting-line">
          <div>
            <strong>电源计划</strong>
            <p className="scope-note">高性能增加耗电与发热</p>
          </div>
          <label>
            <span className="sr-only">电源计划</span>
            <select
              disabled={busy || !power}
              value={power?.mode ?? "custom"}
              onChange={(e) => void setPlan(e.target.value)}
            >
              {power?.mode === "custom" && (
                <option value="custom" disabled>
                  当前自定义计划
                </option>
              )}
              <option value="balanced">平衡</option>
              <option
                value="performance"
                disabled={!power?.performanceAvailable}
              >
                高性能{!power?.performanceAvailable ? "（此设备不支持）" : ""}
              </option>
            </select>
          </label>
        </div>
        <div className="setting-line">
          <div>
            <strong>任务栏修复</strong>
            <p className="scope-note">重启资源管理器，恢复无响应的任务栏</p>
          </div>
          <button
            className="secondary-button"
            disabled={busy}
            onClick={() => setRepairConfirm(true)}
          >
            修复
          </button>
        </div>
        <div className="quick-path-grid">
          {windowsSettingLinks.map((item) => (
            <button
              key={item.target}
              type="button"
              className="quick-path"
              onClick={() => onOpenTarget(item.target)}
            >
              <strong>{item.label}</strong>
              <span>
                {item.label === "重置默认应用"
                  ? "在默认应用页面点击重置 ↗"
                  : "打开设置 ↗"}
              </span>
            </button>
          ))}
        </div>
      </section>
      <ConfirmDialog
        open={repairConfirm}
        title="修复任务栏？"
        description="将重启当前会话的资源管理器，任务栏与桌面会短暂消失，文件夹窗口可能关闭。"
        confirmLabel="重启资源管理器"
        onCancel={() => setRepairConfirm(false)}
        onConfirm={() => void repair()}
      />
      <section className="surface">
        <div className="section-head">
          <h2>维护工具</h2>
        </div>
        <div className="tool-grid">
          {tools.map((tool) => (
            <article key={tool.id} className="soft-card tool-card">
              <h3>{tool.title}</h3>
              <p>{tool.description}</p>
              <button
                className="secondary-button"
                type="button"
                disabled={runningActionId !== null}
                onClick={() => onRunAction(tool.id)}
              >
                {runningActionId === tool.id ? "处理中…" : "运行"}
              </button>
            </article>
          ))}
        </div>
      </section>
    </div>
  );
}

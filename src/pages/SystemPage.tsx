import { useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { MaintenancePanel } from "../MaintenancePanel";
import { windowsToolGroups } from "../content";
import type { WindowsSetting } from "../types";
import { CheckIcon } from "../CheckIcon";
import { ConfirmDialog } from "../ConfirmDialog";

export function SystemPage({
  onOpenTarget,
  active = true,
}: {
  active?: boolean;
  onOpenTarget: (target: string) => void;
}) {
  const [settings, setSettings] = useState<WindowsSetting[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [repairLog, setRepairLog] = useState("");
  const [repairRestart, setRepairRestart] = useState(false);
  const lock = useRef(false);
  const repairLock = useRef(false);
  const activated = useRef(false);
  const [repairBusy, setRepairBusy] = useState(false);
  const [repairMessage, setRepairMessage] = useState("");
  const [repairError, setRepairError] = useState("");
  const [power, setPower] = useState<{
    mode: string;
    performanceAvailable: boolean;
  } | null>(null);
  const [repairConfirm, setRepairConfirm] = useState(false);
  async function refresh() {
    const [items, plan] = await Promise.allSettled([
      invoke<WindowsSetting[]>("get_windows_settings"),
      invoke<{ mode: string; performanceAvailable: boolean }>("get_power_plan"),
    ]);
    if (items.status === "fulfilled") setSettings(items.value);
    if (plan.status === "fulfilled") setPower(plan.value);
    const failures = [items, plan].filter(result => result.status === "rejected").map(result => String(result.reason));
    setError(failures.join("；"));
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
    if (active && !activated.current && isTauri()) {
      activated.current = true;
      void refresh().catch((err) => setError(String(err)));
    }
  }, [active]);
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
      setMessage(id === "autoplay" ? "设置已保存，下一次插入设备时使用。" : id === "block-suggested-apps" ? "设置已保存，后续推荐应用安装使用此配置；已有应用保留。" : "设置已保存并校验；需要刷新的项目已自动刷新。");
    } catch (err) {
      setError(String(err));
    } finally {
      lock.current = false;
      setBusy(false);
    }
  }
  async function repairWindows() {
    if (repairLock.current) return;
    repairLock.current = true;
    setRepairBusy(true);
    setRepairError("");
    setRepairMessage("系统修复中，可能需要数分钟…");
    setRepairLog("");
    setRepairRestart(false);
    try {
      const result = await invoke<{
        success: boolean;
        message: string;
        logs: string;
        restart: boolean;
      }>("repair_windows");
      setRepairMessage(result.message);
      setRepairLog(result.logs);
      setRepairRestart(result.restart);
      if (!result.success) setRepairError(result.message);
    } catch (e) {
      setRepairError(String(e));
      setRepairMessage("");
    } finally {
      repairLock.current = false;
      setRepairBusy(false);
    }
  }
  return (
    <div className="page-stack">
      <MaintenancePanel/>
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
                {item.enabled === null ? <select aria-label={item.label} value="default" disabled={busy || !!item.error}
                  onChange={event => void change(item.id, event.target.value === "on")}>
                  <option value="default" disabled>系统默认</option><option value="on">开启</option><option value="off">关闭</option>
                </select> : <input type="checkbox" role="switch" aria-label={item.label}
                  checked={item.enabled} disabled={busy || !!item.error}
                  onChange={event => void change(item.id, event.target.checked)}/>}
                <span><strong>{item.label}</strong><small className="setting-description">{item.description}</small>
                  {item.error ? <small className="inline-error">{item.error}</small> : item.enabled === null && <small className="scope-note">未显式设置，使用系统默认</small>}
                </span>
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
      </section>
      {windowsToolGroups.map(group => <section className="surface" key={group.title}>
        <div className="section-head"><h2>{group.title}</h2></div>
        <div className="quick-path-grid">{group.items.map(item => <button
          key={item.target} type="button" className="quick-path system-tool"
          onClick={() => onOpenTarget(item.target)}>
          <CheckIcon kind={item.target.includes("gaming") ? "game" : item.target.includes("power") ? "power" : item.target.includes("notifications") ? "notifications" : group.icon}/><div><strong>{item.label}</strong><span>{item.detail}</span></div><span aria-hidden="true">↗</span>
        </button>)}</div>
      </section>)}
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
          <h2>系统修复</h2>
        </div>
        <p className="scope-note">
          修复 Windows 组件并检查系统文件 · 需要管理员授权，可能使用 Windows
          更新下载修复文件
        </p>
        <button
          className="secondary-button"
          disabled={repairBusy}
          onClick={() => void repairWindows()}
        >
          {repairBusy ? "修复中…" : "检查并修复"}
        </button>
        {repairMessage && <p className="scope-note" role="status">{repairMessage}</p>}
        {repairError && <p className="inline-error" role="alert">{repairError}</p>}
        {repairRestart && (
          <p className="scope-note">
            系统要求重启电脑后完成修复，请保存工作后重启。
          </p>
        )}
        {repairLog && (
          <details>
            <summary>修复记录</summary>
            <pre className="system-repair-log">{repairLog}</pre>
          </details>
        )}
      </section>
    </div>
  );
}

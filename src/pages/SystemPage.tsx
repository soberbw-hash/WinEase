import { useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MaintenancePanel } from "../MaintenancePanel";
import { windowsToolGroups } from "../content";
import { CheckIcon } from "../CheckIcon";
import { ConfirmDialog } from "../ConfirmDialog";

export function SystemPage({
  onOpenTarget,
  active: _active = true,
}: {
  active?: boolean;
  onOpenTarget: (target: string) => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [repairLog, setRepairLog] = useState("");
  const [repairRestart, setRepairRestart] = useState(false);
  const lock = useRef(false);
  const repairLock = useRef(false);
  const [repairBusy, setRepairBusy] = useState(false);
  const [repairMessage, setRepairMessage] = useState("");
  const [repairError, setRepairError] = useState("");
  const [repairConfirm, setRepairConfirm] = useState(false);
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
        <div className="section-head"><h2>任务栏修复</h2></div>
        {error && <p className="inline-error" role="alert">{error}</p>}
        {message && <p className="scope-note" role="status">{message}</p>}
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

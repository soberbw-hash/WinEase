import { useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ConfirmDialog } from "./ConfirmDialog";
import { CheckIcon } from "./CheckIcon";
import type { MaintenanceReport } from "./communityTypes";
const actions = [
  {id: "update-services", label: "修复更新服务", scope: "检查被禁用的 BITS 与 Windows Update，备份后改为手动并启动。已有组织管理策略时停止；不删除更新文件，不保证所有更新错误都能解决。"},
  {id: "winget-register", label: "修复软件安装工具", scope: "重新注册当前用户的应用安装程序，保留数据，再检查 WinGet 命令。应用缺失时需在商店安装。"},
  {id: "winget-source", label: "刷新软件安装列表", scope: "联网更新官方 winget 源，保留自定义源与代理配置。"},
  {id: "store-register", label: "修复商店注册", scope: "重新注册当前用户的 Microsoft Store，保留应用数据并复查包状态。"},
  {id: "store-cache", label: "重建商店缓存", scope: "运行 Windows 商店缓存重置。可能打开商店；保留已安装应用。完成后仍需验证原来的下载或登录问题。"},
  {id: "restore-update-services", label: "恢复更新服务", scope: "按 WinEase 保存的记录恢复更新服务原来的禁用配置；之后更新可能再次无法使用。"},
];
export function MaintenancePanel() {
  const [report, setReport] = useState<MaintenanceReport | null>(null);
  const [checking, setChecking] = useState(false), [busy, setBusy] = useState("");
  const [error, setError] = useState(""), [message, setMessage] = useState("");
  const [pending, setPending] = useState<typeof actions[number] | null>(null);
  const checkLock = useRef(false), repairLock = useRef(false);
  async function check() {
    if (checkLock.current) return; checkLock.current = true;setChecking(true);setError("");
    try {setReport(await invoke<MaintenanceReport>("check_maintenance"));} catch(e) {setError(String(e));} finally {checkLock.current = false;setChecking(false);}
  }
  async function repair() {
    const action = pending;setPending(null);if (!action || repairLock.current) return;
    repairLock.current = true;setBusy(action.id);setError("");setMessage("");
    try {
      if (action.id === "restore-update-services") {
        const result = await invoke<{success: boolean; message: string; logs: string}>("restore_update_services", {confirmed: true});
        if (!result.success) throw result.message;setMessage(result.message);
      } else setMessage(await invoke<string>("repair_maintenance", {action: action.id, confirmed: true}));
      setReport(await invoke<MaintenanceReport>("check_maintenance"));
    } catch(e) {setError(String(e));} finally {repairLock.current = false;setBusy("");}
  }
  return <section className="surface">
    <div className="section-head"><h2><CheckIcon kind="repair"/>更新与应用安装</h2><button className="secondary-button" disabled={checking || Boolean(busy)} onClick={() => void check()}>{checking ? "检查中…" : "检查状态"}</button></div>
    <p className="scope-note">检查系统更新、软件安装工具和应用商店，处理更新或安装失败。</p>
    {error && <p role="alert" className="inline-error">{error}</p>}{message && <p role="status" className="scope-note">{message}</p>}
    {report?.checks.map(row => <div className="manager-row" key={row.id}><CheckIcon kind="health"/><div className="manager-row__text"><strong>{row.title} <small>{row.status === "healthy" ? "已核实" : row.status === "unknown" ? "未核实" : "需关注"}</small></strong><small>{row.detail}</small></div>{row.repair && <button className="secondary-button" disabled={Boolean(busy)} onClick={() => setPending(actions.find(a => a.id === row.repair) ?? null)}>修复</button>}</div>)}
    {report && !report.checks.some(row => row.status === "attention") && <p className="scope-note">未发现已检测范围内的故障；下载与登录问题可按下方用途处理。</p>}
    <details><summary>按问题修复与恢复</summary><div className="quick-path-grid">{actions.filter(action => action.id !== "restore-update-services" || report?.canRestoreServices).map(action => <button className="quick-path system-tool" key={action.id} disabled={Boolean(busy)} onClick={() => setPending(action)}><CheckIcon kind="repair"/><div><strong>{busy === action.id ? "处理中…" : action.label}</strong><small>{action.id === "winget-source" ? "从官方目录重新获取软件信息" : action.id === "winget-register" ? "WinGet 是系统的软件安装工具" : action.id === "store-register" ? "处理应用商店打不开或注册异常" : action.id === "store-cache" ? "处理商店下载或登录异常" : action.id === "update-services" ? "重新启动被停用的系统更新服务" : "恢复更新服务优化前的配置"}</small></div></button>)}</div></details>
    <ConfirmDialog open={pending !== null} title={`${pending?.label ?? "修复"}？`} description={pending?.scope ?? ""} confirmLabel="确认执行" onCancel={() => setPending(null)} onConfirm={() => void repair()}/>
  </section>;
}

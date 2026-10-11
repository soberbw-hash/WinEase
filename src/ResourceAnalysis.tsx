import { useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ApplicationIcon } from "./ApplicationIcon";
import { CheckIcon } from "./CheckIcon";
import { formatBytes } from "./format";
import type { ResourceReport } from "./communityTypes";

export function ResourceAnalysis({visible = true}: {visible?: boolean}) {
  const [report, setReport] = useState<ResourceReport | null>(null);
  const [busy, setBusy] = useState(false), [error, setError] = useState("");
  const lock = useRef(false);
  async function analyze() {
    if (lock.current) return;
    lock.current = true; setBusy(true); setError("");
    try {setReport(await invoke<ResourceReport>("analyze_resources"));}
    catch (e) {setError(String(e));}
    finally {lock.current = false; setBusy(false);}
  }
  return <section className="surface" hidden={!visible}>
    <div className="section-head"><h2><CheckIcon kind="startup"/>启动与后台分析</h2><button className="secondary-button" disabled={busy} onClick={() => void analyze()}>{busy ? "采样中…" : report ? "重新分析" : "开始分析"}</button></div>
    {error && <p role="alert" className="inline-error">{error}</p>}
    {!report && <p className="scope-note">连续采样 CPU、内存，读取开机记录。</p>}
    {report && <>
      <p className="scope-note">{report.boot ? `最近记录的开机耗时 ${(report.boot.durationMs / 1000).toFixed(1)} 秒 · ${new Date(report.boot.time).toLocaleString()}` : "开机耗时未能核实"} · 资源采样 {report.seconds} 秒</p>
      {report.bootError && <details><summary>开机记录未完整读取</summary><p className="scope-note">{report.bootError}</p></details>}
      {report.startup.length > 0 && <details><summary>开机延迟记录 · {report.startup.length} 项</summary>{report.startup.map((item, i) => <div className="manager-row" key={i}><ApplicationIcon target={item.path ?? ""}/><div className="manager-row__text"><strong>{item.name ?? "未命名应用"}</strong><small title={item.path ?? ""}>{item.path}</small></div><span className="tabular">{item.delayMs === null ? "未核实" : `额外延迟 ${(item.delayMs / 1000).toFixed(1)} 秒`}</span></div>)}</details>}
      {report.groups.slice(0, 15).map(item => <div className="manager-row" key={item.path}><ApplicationIcon target={item.path}/><div className="manager-row__text"><strong>{item.name} <small>{item.processCount} 个进程</small></strong><small title={item.path}>{item.attention ? "占用较高；先检查工作负载，再决定关闭或停用启动" : "本次采样未达高占用阈值"}</small></div><span className="tabular">CPU {item.cpuPercent.toFixed(1)}% · {formatBytes(item.memoryBytes)}</span></div>)}
      {report.groups.length === 0 && <p className="scope-note">没有足够的连续采样，点击重新分析。</p>}
    </>}
  </section>;
}

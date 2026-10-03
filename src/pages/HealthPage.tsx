import { useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { CheckIcon } from "../CheckIcon";
import type { HealthReport } from "../optimizationTypes";
export function HealthPage({onNavigate}: {onNavigate: (id: string) => void}) {
  const [data, setData] = useState<HealthReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const lock = useRef(false);
  async function check() {
    if (lock.current) return;
    lock.current = true; setBusy(true); setError("");
    try {setData(await invoke<HealthReport>("health_check"));}
    catch (err) {setError(String(err));}
    finally {lock.current = false; setBusy(false);}
  }
  useEffect(() => {if (isTauri()) void check();}, []);
  return <section className="surface">
    <div className="section-head"><h2>电脑体检</h2><button className="secondary-button" disabled={busy} onClick={() => void check()}>{busy ? "检查中…" : "重新检查"}</button></div>
    {error && <p className="inline-error" role="alert">{error}</p>}
    {!data && <p role="status">{busy ? "正在检查系统健康…" : "检查系统空间、防护、设备与可靠性"}</p>}
    {data?.checks.map(row => <div className="optimization-item__row" key={row.id}>
      <CheckIcon kind="health"/><div className="optimization-item__text"><strong>{row.title}</strong><p>{row.detail}</p></div>
      <span className={`pill optimization-status-${row.status}`}>{row.status === "attention" ? "需关注" : row.status === "unknown" ? "未核实" : row.status === "healthy" ? "正常" : "待确认"}</span>
      {row.target && <button className="ghost-button" onClick={() => onNavigate(row.target!)}>查看处理 ↗</button>}
    </div>)}
  </section>;
}

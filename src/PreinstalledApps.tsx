import { useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ConfirmDialog } from "./ConfirmDialog";
import { ApplicationIcon } from "./ApplicationIcon";
import { CheckIcon } from "./CheckIcon";
import { formatBytes } from "./format";
import type { AppPackage } from "./communityTypes";
export function PreinstalledApps({visible = true, onOpenTarget}: {visible?: boolean; onOpenTarget: (target: string) => void}) {
  const [rows, setRows] = useState<AppPackage[]>([]), [loaded, setLoaded] = useState(false);
  const [search, setSearch] = useState(""), [all, setAll] = useState(false);
  const [busy, setBusy] = useState(false), [error, setError] = useState(""), [message, setMessage] = useState("");
  const [pending, setPending] = useState<AppPackage | null>(null);
  const lock = useRef(false);
  async function refresh() {setRows(await invoke<AppPackage[]>("list_preinstalled_apps")); setLoaded(true);}
  async function scan() {
    if (lock.current) return;
    lock.current = true;setBusy(true);setError("");
    try {await refresh();} catch(e) {setError(String(e));} finally {lock.current = false;setBusy(false);}
  }
  async function apply() {
    const item = pending;setPending(null);if (!item || lock.current) return;
    lock.current = true;setBusy(true);setError("");setMessage("");
    try {setMessage(await invoke<string>("manage_preinstalled_app", {name: item.name, fullName: item.fullName, restore: item.removed, confirmed: true})); await refresh();}
    catch(e) {setError(String(e));} finally {lock.current = false;setBusy(false);}
  }
  const visibleRows = rows.filter(item => (all || item.removable || item.removed) && `${item.label} ${item.name}`.toLowerCase().includes(search.toLowerCase()));
  return <section className="surface" hidden={!visible}>
    <div className="section-head"><h2><CheckIcon kind="files"/>预装应用</h2><button className="secondary-button" disabled={busy} onClick={() => void scan()}>{busy ? "处理中…" : loaded ? "重新扫描" : "扫描应用"}</button></div>
    <p className="scope-note">仅处理当前用户。卸载可能删除应用数据；恢复记录用于重新安装，不是数据备份。</p>
    {error && <p className="inline-error" role="alert">{error}</p>}{message && <p className="scope-note" role="status">{message}</p>}
    <div className="community-filter"><label><input type="checkbox" checked={all} onChange={e => setAll(e.target.checked)}/> 显示受保护应用</label><input aria-label="搜索预装应用" type="search" placeholder="搜索应用" value={search} onChange={e => setSearch(e.target.value)}/></div>
    {visibleRows.map(item => <div className="manager-row" key={item.name}>
      <ApplicationIcon target={item.iconTarget ?? ""}/><div className="manager-row__text"><strong>{item.label} {item.removed && <small>已移除</small>}</strong><small>{item.purpose}</small><details><summary>版本与依赖</summary><p className="scope-note">{item.version} · {item.name}<br/>{item.dependencies.length ? item.dependencies.join("、") : "无已记录依赖"}</p></details></div>
      <span className="tabular">{item.removed ? "" : item.sizeBytes === null ? "大小未核实" : `${item.sizeLimited ? "已统计 " : ""}${formatBytes(item.sizeBytes)}`}</span>
      {item.removed ? <><button className="secondary-button" disabled={busy} onClick={() => setPending(item)}>尝试恢复</button><button className="ghost-button" onClick={() => onOpenTarget(`ms-windows-store://search/?query=${encodeURIComponent(item.label)}`)}>商店重装</button></> : <button className="secondary-button" disabled={busy || !item.removable} onClick={() => setPending(item)}>{item.removable ? "卸载" : "受保护"}</button>}
    </div>)}
    {loaded && visibleRows.length === 0 && <p className="scope-note">没有匹配的可选应用，可重新扫描或显示受保护应用。</p>}
    <ConfirmDialog open={pending !== null} title={pending?.removed ? "恢复应用？" : "卸载应用？"} description={pending ? `${pending.label}：${pending.removed ? "尝试重新注册本机留存安装包；若包已移除，需通过商店重装。应用数据不保证恢复。" : "只从当前用户移除，可能删除应用数据。已保存安装信息用于尝试恢复；其他用户与系统预配保留。"}` : ""} confirmLabel={pending?.removed ? "尝试恢复" : "确认卸载"} onCancel={() => setPending(null)} onConfirm={() => void apply()}/>
  </section>;
}

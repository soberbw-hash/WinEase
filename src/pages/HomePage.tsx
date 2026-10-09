import { useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ApplicationIcon } from "../ApplicationIcon";
import { CheckIcon } from "../CheckIcon";
import { ConfirmDialog } from "../ConfirmDialog";
import { formatBytes } from "../format";
import { useOptimization } from "../hooks/useOptimization";
import type { OptimizationItem, OptimizationSection } from "../optimizationTypes";

type Files = {files: Array<{id: number; name: string; path: string; sizeBytes: number}>; total: number};
const labels: Record<string, string> = {recommended: "建议优化", optional: "需确认", attention: "需关注", healthy: "正常", info: "保留", unknown: "未核实"};
export function HomePage({onQuickAction}: {onQuickAction: (id: string) => void}) {
  const flow = useOptimization();
  const {report, selected} = flow;
  const [confirm, setConfirm] = useState(false);
  const [expanded, setExpanded] = useState<string[]>([]);
  const [details, setDetails] = useState<Record<string, Files>>({});
  const [detailErrors, setDetailErrors] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState<string[]>([]);
  const [visible, setVisible] = useState<Record<string, number>>({});
  const [search, setSearch] = useState("");
  const detailLock = useRef(new Set<string>());
  const activeReport = useRef(report?.id);
  activeReport.current = report?.id;
  const busy = flow.submitting || report?.phase === "checking" || report?.phase === "optimizing";
  const mutable = report?.phase === "ready" || report?.phase === "checking";
  const rows = report?.sections.flatMap(section => section.items) ?? [];
  const selection = new Set(selected);
  const chosen = rows.filter(row => row.selectable && selection.has(row.id));
  const cacheBytes = chosen.filter(row => row.actionLabel === "清理缓存").reduce((sum, row) => sum + row.sizeBytes, 0);
  const fileBytes = chosen.filter(row => row.actionLabel === "移到回收站").reduce((sum, row) => sum + row.sizeBytes, 0);
  const startupCount = chosen.filter(row => row.actionLabel === "关闭开机启动").length;
  const networkCount = chosen.filter(row => row.actionLabel === "备份并修复").length;
  const finished = report?.sections.filter(section => section.status !== "checking").length ?? 0;
  async function check() {
    if (busy) return;
    setExpanded([]); setDetails({}); setDetailErrors({}); setLoading([]); setVisible({}); setSearch("");
    await flow.check();
  }
  async function files(row: OptimizationItem, offset = 0) {
    if (!report) return;
    const id = report.id, key = `${id}:${row.id}`;
    if (detailLock.current.has(key)) return;
    detailLock.current.add(key); setLoading(ids => [...ids, row.id]);
    setDetailErrors(errors => ({...errors, [row.id]: ""}));
    try {
      const data = await invoke<Files>("optimization_files", {reportId: id, itemId: row.id, offset});
      if (activeReport.current === id) setDetails(current => ({...current, [row.id]: {total: data.total, files: offset ? [...(current[row.id]?.files ?? []), ...data.files] : data.files}}));
    } catch (err) {if (activeReport.current === id) setDetailErrors(current => ({...current, [row.id]: String(err)}));}
    finally {detailLock.current.delete(key); if (activeReport.current === id) setLoading(ids => ids.filter(value => value !== row.id));}
  }
  function expand(row: OptimizationItem) {
    setExpanded(ids => ids.includes(row.id) ? ids.filter(id => id !== row.id) : [...ids, row.id]);
    if (!details[row.id]) void files(row);
  }
  function renderItem(row: OptimizationItem, section: string) {
    const result = report?.outcomes.find(outcome => outcome.itemIds.includes(row.id));
    return <div className="optimization-item" key={row.id}>
      <div className="optimization-item__row">
        {row.selectable ? <input type="checkbox" aria-label={`选择 ${row.title}`} checked={selected.includes(row.id)} disabled={!mutable || flow.submitting} onChange={event => flow.toggle(row.id, event.target.checked)}/> : <span className="optimization-check-spacer"/>}
        {row.iconTarget ? <ApplicationIcon target={row.iconTarget} command={row.iconKind === "command"} file={row.iconKind === "file"} fallback={<CheckIcon kind={row.iconKind === "file" ? "files" : section}/>}/> : <CheckIcon kind={section}/>}
        <div className="optimization-item__text">
          <strong>{row.title}</strong>
          <p>{row.detail}</p>
          {result && <p className={result.status === "failed" ? "inline-error" : "optimization-success"}>{result.message}</p>}
        </div>
        <div className="optimization-item__value">
          {row.sizeBytes > 0 && <strong className="tabular">{formatBytes(row.sizeBytes)}</strong>}
          {row.hasFiles && <small>{row.fileCount} 个文件</small>}
          <span className={`pill optimization-status-${result?.status ?? row.status}`}>{result ? result.status === "success" ? "已处理" : "未完成" : labels[row.status] ?? row.status}</span>
        </div>
        <div className="optimization-item__buttons">
          {row.hasFiles && <button className="ghost-button" disabled={!!result && !details[row.id]} aria-expanded={expanded.includes(row.id)} aria-controls={`inspection-${row.id}`} onClick={() => expand(row)}>{expanded.includes(row.id) ? "收起文件" : result && !details[row.id] ? "已处理文件" : "查看文件"}</button>}
          {row.target && <button className="ghost-button" onClick={() => onQuickAction(row.target!)}>{section === "files" ? "打开目录" : "查看处理"} ↗</button>}
        </div>
      </div>
      {expanded.includes(row.id) && <div className="optimization-files" id={`inspection-${row.id}`}>
        {detailErrors[row.id] && <p className="inline-error" role="alert">{detailErrors[row.id]} <button className="ghost-button" onClick={() => void files(row)}>重试</button></p>}
        {details[row.id]?.files.map(file => <div className="optimization-file" key={file.id}>
          <ApplicationIcon target={file.path} file fallback={<CheckIcon kind="files"/>}/>
          <div><strong>{file.name}</strong><small title={file.path}>{file.path}</small></div>
          <span className="tabular">{formatBytes(file.sizeBytes)}</span>
        </div>)}
        {result && <p className="scope-note">以下为检查时的文件明细。</p>}
        {loading.includes(row.id) ? <p role="status">读取文件明细…</p> : !result && details[row.id] && details[row.id].files.length < details[row.id].total && <button className="secondary-button" onClick={() => void files(row, details[row.id].files.length)}>更多文件（{details[row.id].files.length}/{details[row.id].total}）</button>}
      </div>}
    </div>;
  }
  function renderSection(section: OptimizationSection) {
    const main = section.items.filter(row => !["healthy", "info"].includes(row.status));
    const retained = section.items.filter(row => ["healthy", "info"].includes(row.status));
    const filtered = section.id === "files" && search.trim() ? main.filter(row => `${row.title} ${row.detail}`.toLowerCase().includes(search.trim().toLowerCase())) : main;
    const count = visible[section.id] ?? 30;
    return <section className="surface optimization-section" id={`check-${section.id}`} key={section.id} aria-labelledby={`heading-${section.id}`}>
      <div className="section-head"><div className="optimization-section__title"><CheckIcon kind={section.id}/><h2 id={`heading-${section.id}`}>{section.title}</h2></div><span className="pill">{section.status === "checking" ? "检查中" : section.status === "error" ? "未完成" : `${section.items.length} 项`}</span></div>
      <p className={section.status === "error" ? "inline-error" : "scope-note"}>{section.summary}</p>
      {section.status === "checking" && <div className="optimization-skeleton" role="status">正在读取{section.title}…</div>}
      {section.id === "files" && section.items.length > 0 && <input className="management-search" aria-label="搜索大文件" placeholder="搜索文件名或路径" value={search} onChange={event => setSearch(event.target.value)}/>}
      {filtered.slice(0, count).map(row => renderItem(row, section.id))}
      {filtered.length > count && <button className="secondary-button" onClick={() => setVisible(current => ({...current, [section.id]: count + 50}))}>显示更多（{count}/{filtered.length}）</button>}
      {retained.length > 0 && <details className="optimization-retained"><summary>正常与保留项目 · {retained.length} 项</summary>{retained.map(row => renderItem(row, section.id))}</details>}
      {section.id === "cleaning" && section.status === "complete" && <button className="ghost-button" onClick={() => onQuickAction("ms-settings:storagesense")}>Windows 更新及系统清理 ↗</button>}
      {section.id === "startup" && section.status === "complete" && <p className="scope-note">关闭的启动项可在开机管理中恢复；不会结束正在运行的应用。</p>}
    </section>;
  }
  const confirmDescription = [
    cacheBytes > 0 ? `直接删除所选缓存，预计 ${formatBytes(cacheBytes)}，无法撤销。` : "",
    fileBytes > 0 ? `所选个人大文件共 ${formatBytes(fileBytes)} 将移到回收站；请确认用途。` : "",
    startupCount ? `关闭 ${startupCount} 个应用的开机启动，可恢复。` : "",
    networkCount ? "备份并修复已勾选的网络问题，可能弹出管理员授权；如项目注明，可能短暂断网或需要重启。" : "",
  ].filter(Boolean).join(" ");
  return <div className="page-stack optimization-home">
    {flow.error && <p className="inline-error" role="alert">{flow.error}</p>}
    {!report ? <section className="surface optimization-welcome">
      <div className="optimization-welcome__icon"><CheckIcon kind="health"/></div>
      <h2>检查与优化</h2>
      <p>清理 · 空间 · 体检 · 网络 · 开机</p>
      <button className="primary-button optimization-primary" disabled={flow.submitting} onClick={() => void check()}>{flow.submitting ? "开始检查…" : "一键检查"}</button>
      <small>先检查，再确认优化</small>
    </section> : <>
      <section className="surface optimization-summary" aria-label="检查概览">
        <div className="optimization-summary__top"><div>
          <h2>{report.phase === "checking" ? "正在检查" : report.phase === "optimizing" ? "正在优化" : report.phase === "complete" ? "优化结果" : report.phase === "expired" ? "检查结果已过期" : "检查结果"}</h2>
          <p aria-live="polite">{report.phase === "checking" ? `${finished} / 5 项检查已完成` : report.phase === "optimizing" ? `${report.completedActions} / ${report.totalActions} 项已处理` : report.phase === "complete" ? `${report.completedActions} 项已处理 · ${report.outcomes.filter(result => result.status === "failed").length} 项操作未完整完成` : report.phase === "expired" ? "重新检查后可继续优化" : chosen.length ? `已选 ${chosen.length} 项${cacheBytes > 0 ? ` · 缓存 ${formatBytes(cacheBytes)}` : ""}${fileBytes > 0 ? ` · 大文件 ${formatBytes(fileBytes)}` : ""}` : "暂无已选优化项，可查看下方建议"}</p>
        </div><div className="button-row">
          {report.phase === "ready" && <button className="ghost-button" disabled={flow.submitting} onClick={flow.recommendations}>使用建议项</button>}
          <button className="secondary-button" disabled={busy} onClick={() => void check()}>重新检查</button>
          {report.phase !== "complete" && report.phase !== "expired" && <button className="primary-button optimization-primary" disabled={busy || report.phase !== "ready" || !chosen.length} onClick={() => setConfirm(true)}>{report.phase === "optimizing" ? "优化中…" : "一键优化"}</button>}
        </div></div>
        <div className="optimization-stages">{report.sections.map(section => <a href={`#check-${section.id}`} key={section.id}><CheckIcon kind={section.id}/><span>{section.title}</span><small>{section.status === "checking" ? "检查中" : section.status === "error" ? "未完成" : "已检查"}</small></a>)}</div>
        {(report.phase === "checking" || report.phase === "optimizing") && <progress aria-label={report.phase === "checking" ? "检查进度" : "优化进度"} max={report.phase === "checking" ? 5 : report.totalActions || 1} value={report.phase === "checking" ? finished : report.completedActions}/>}
      </section>
      {report.outcomes.length > 0 && <section className="surface" aria-label="执行结果"><h2>本次处理</h2>{report.outcomes.map((result,index) => <div className="optimization-outcome" key={index}><strong>{result.title}</strong><p className={result.status === "failed" ? "inline-error" : "optimization-success"}>{result.message}</p></div>)}</section>}
      {report.sections.map(renderSection)}
    </>}
    <ConfirmDialog open={confirm} title={`优化已选的 ${chosen.length} 个项目？`} description={confirmDescription} confirmLabel="确认并优化" onCancel={() => setConfirm(false)} onConfirm={() => {setConfirm(false); void flow.optimize();}}/>
  </div>;
}

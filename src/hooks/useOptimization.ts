import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { OptimizationReport } from "../optimizationTypes";

export function useOptimization() {
  const [report, setReport] = useState<OptimizationReport | null>(null);
  const [selected, setSelected] = useState<string[]>([]);
  const [error, setError] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const current = useRef<OptimizationReport | null>(null);
  const seen = useRef(new Set<string>());
  const lock = useRef(false);
  function accept(next: OptimizationReport) {
    if (current.current && (current.current.id !== next.id || current.current.revision > next.revision)) return;
    const rows = next.sections.flatMap(section => section.items);
    const defaults = rows.filter(row => row.defaultSelected && row.selectable && !seen.current.has(row.id)).map(row => row.id);
    rows.forEach(row => seen.current.add(row.id));
    const selectable = new Set(rows.filter(row => row.selectable).map(row => row.id));
    setSelected(ids => [...new Set([...ids, ...defaults])].filter(id => selectable.has(id)));
    current.current = next;
    setReport(next);
  }
  async function check() {
    if (lock.current || ["checking", "optimizing"].includes(current.current?.phase ?? "")) return;
    lock.current = true; setSubmitting(true); setError("");
    try {
      const next = await invoke<OptimizationReport>("start_optimization_check");
      current.current = null; seen.current.clear(); setSelected([]); accept(next);
    } catch (err) { setError(String(err)); }
    finally { lock.current = false; setSubmitting(false); }
  }
  async function optimize() {
    if (lock.current || current.current?.phase !== "ready" || !selected.length) return;
    lock.current = true; setSubmitting(true); setError("");
    try {
      accept(await invoke<OptimizationReport>("start_optimization", {reportId: current.current.id, itemIds: selected, confirmed: true}));
    } catch (err) { setError(String(err)); }
    finally { lock.current = false; setSubmitting(false); }
  }
  useEffect(() => {
    if (!report || !["checking", "ready", "optimizing"].includes(report.phase)) return;
    const id = report.id;
    let disposed = false;
    let timer: ReturnType<typeof setTimeout>;
    async function poll() {
      try {
        const next = await invoke<OptimizationReport | null>("optimization_status", {reportId: id, revision: current.current?.revision});
        if (!disposed && next) { accept(next); setError(""); }
      } catch (err) { if (!disposed) setError(String(err)); }
      finally {
        if (!disposed) timer = setTimeout(() => void poll(), current.current?.phase === "ready" ? 5000 : 650);
      }
    }
    timer = setTimeout(() => void poll(), 200);
    return () => { disposed = true; clearTimeout(timer); };
  }, [report?.id, report?.phase]);
  function toggle(id: string, checked: boolean) {
    setSelected(ids => checked ? [...new Set([...ids, id])] : ids.filter(item => item !== id));
  }
  function recommendations() {
    if (!report) return;
    setSelected(report.sections.flatMap(section => section.items).filter(row => row.defaultSelected && row.selectable).map(row => row.id));
  }
  return {report, selected, error, submitting, check, optimize, toggle, recommendations};
}

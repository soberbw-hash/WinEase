import { useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { WindowsSetting } from "./types";
import { ToggleSwitch } from "./ToggleSwitch";

export function OptimizationSettings({disabled = false, revision, onChanged}: {
  disabled?: boolean; revision?: string; onChanged: () => void;
}) {
  const [settings, setSettings] = useState<WindowsSetting[]>([]);
  const [pending, setPending] = useState<string[]>([]);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const locks = useRef(new Set<string>());
  async function refresh() {
    try {
      const items = await invoke<WindowsSetting[]>("get_optimization_presets");
      if (!Array.isArray(items)) throw "优化状态未核实，请刷新重试";
      setSettings(items); setErrors(current => ({...current, settings: ""}));
    } catch(error) {setSettings([]); setErrors(current => ({...current, settings: String(error)}));}
  }
  useEffect(() => {if (isTauri()) void refresh();}, [revision]);
  async function change(id: string, enabled: boolean) {
    if (disabled || locks.current.has(id)) return;
    locks.current.add(id); setPending(ids => [...ids, id]);
    setErrors(current => ({...current, [id]: ""}));
    try {
      await invoke("set_optimization_preset", {id, enabled: enabled === true, confirmed: true});
      onChanged(); await refresh();
    } catch (error) {onChanged(); setErrors(current => ({...current, [id]: String(error)}));}
    finally {locks.current.delete(id); setPending(ids => ids.filter(value => value !== id));}
  }
  return <section className="optimization-options__body" aria-label="优化设置">
    <div className="section-head">
      <button className="ghost-button" disabled={pending.length > 0} onClick={() => void refresh()}>刷新状态</button></div>
    {errors.settings && <p className="inline-error" role="alert">{errors.settings}</p>}
    {!settings.length && !errors.settings && <p className="scope-note">{isTauri() ? "读取设置中…" : "请在桌面应用中读取设置"}</p>}
    {settings.map(item => <div className="setting-line optimization-setting" key={item.id}>
      <div className="optimization-setting__text"><strong>{item.label}</strong><small className="setting-description">{item.description}</small>
        {item.enabled === true && !item.canRestore && <small className="setting-description">已由系统或其他工具设置，关闭使用默认方案</small>}
        {(item.error || errors[item.id]) && <small className="inline-error" role="alert">{item.error || errors[item.id]}</small>}</div>
      <div className="optimization-setting__controls">
        <ToggleSwitch label={item.label} checked={item.enabled === true}
        state={pending.includes(item.id) ? "处理中" : item.error ? "未核实" : item.enabled === null ? "未核实" : item.enabled ? "已优化" : "未优化"}
        disabled={disabled || pending.includes(item.id) || !!item.error} onChange={enabled => void change(item.id, enabled)}/>
      </div>
    </div>)}
  </section>;
}

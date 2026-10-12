import { useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { WindowsSetting } from "./types";
import { ToggleSwitch } from "./ToggleSwitch";
import { CheckIcon } from "./CheckIcon";

type Power = {mode: string; performanceAvailable: boolean};
export function OptimizationSettings({disabled = false, revision, onChanged}: {
  disabled?: boolean; revision?: string; onChanged: () => void;
}) {
  const [settings, setSettings] = useState<WindowsSetting[]>([]);
  const [power, setPower] = useState<Power | null>(null);
  const [pending, setPending] = useState<string[]>([]);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const locks = useRef(new Set<string>());
  async function refresh() {
    const [items, plan] = await Promise.allSettled([
      invoke<WindowsSetting[]>("get_windows_settings"), invoke<Power>("get_power_plan"),
    ]);
    const validSettings = items.status === "fulfilled" && Array.isArray(items.value);
    const validPower = plan.status === "fulfilled" && plan.value && typeof plan.value.mode === "string" && typeof plan.value.performanceAvailable === "boolean";
    setSettings(validSettings ? items.value : []);
    setPower(validPower ? plan.value : null);
    setErrors(current => ({...current,
      settings: items.status === "rejected" ? String(items.reason) : validSettings ? "" : "设置状态未核实，请刷新重试",
      power: plan.status === "rejected" ? String(plan.reason) : validPower ? "" : "电源状态未核实，请刷新重试",
    }));
  }
  useEffect(() => {if (isTauri()) void refresh();}, [revision]);
  async function change(id: string, enabled?: boolean) {
    if (disabled || locks.current.has(id)) return;
    locks.current.add(id); setPending(ids => [...ids, id]);
    setErrors(current => ({...current, [id]: ""}));
    try {
      if (id === "power") await invoke("set_power_plan", {mode: enabled ? "performance" : "balanced"});
      else await invoke(enabled === undefined ? "restore_windows_setting" : "set_windows_setting", {id, enabled});
      onChanged(); await refresh();
    } catch (error) {onChanged(); setErrors(current => ({...current, [id]: String(error)}));}
    finally {locks.current.delete(id); setPending(ids => ids.filter(value => value !== id));}
  }
  return <section id="optimization-settings" className="surface" aria-label="优化设置">
    <div className="section-head"><h2><CheckIcon kind="preferences"/>优化设置</h2>
      <button className="ghost-button" disabled={pending.length > 0} onClick={() => void refresh()}>刷新状态</button></div>
    {errors.settings && <p className="inline-error" role="alert">{errors.settings}</p>}
    {!settings.length && !errors.settings && <p className="scope-note">{isTauri() ? "读取设置中…" : "请在桌面应用中读取设置"}</p>}
    {settings.map(item => <div className="setting-line optimization-setting" key={item.id}>
      <div className="optimization-setting__text"><strong>{item.label}</strong><small className="setting-description">{item.description}</small>
        {(item.error || errors[item.id]) && <small className="inline-error" role="alert">{item.error || errors[item.id]}</small>}</div>
      <div className="optimization-setting__controls">
        {item.canRestore && <button className="ghost-button" disabled={disabled || pending.includes(item.id)} onClick={() => void change(item.id)}>恢复原设置</button>}
        <ToggleSwitch label={item.label} checked={item.enabled === true}
        state={pending.includes(item.id) ? "处理中" : item.error ? "未核实" : item.enabled === null ? "系统默认" : undefined}
        disabled={disabled || pending.includes(item.id) || !!item.error} onChange={enabled => void change(item.id, enabled)}/>
      </div>
    </div>)}
    <div className="setting-line optimization-setting">
      <div className="optimization-setting__text"><strong>高性能</strong><small className="setting-description">开启提高性能与耗电，关闭使用默认平衡计划</small>
        {power?.mode === "custom" && <small className="scope-note">当前使用自定义电源计划</small>}
        {power && !power.performanceAvailable && power.mode !== "performance" && <small className="scope-note">此设备没有可用的高性能计划</small>}
        {errors.power && <small className="inline-error" role="alert">{errors.power}</small>}</div>
      <ToggleSwitch label="高性能" checked={power?.mode === "performance"}
        state={pending.includes("power") ? "处理中" : !power ? "未核实" : power.mode === "custom" ? "自定义" : power.mode === "performance" ? "高性能" : "默认"}
        disabled={disabled || pending.includes("power") || !power || (power.mode !== "performance" && !power.performanceAvailable)}
        onChange={enabled => void change("power", enabled)}/>
    </div>
  </section>;
}

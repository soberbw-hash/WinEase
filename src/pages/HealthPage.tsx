import { useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { formatBytes } from "../format";
type Health = {
  memoryPercent: number;
  drives: Array<{ name: string; freeBytes: number; totalBytes: number }>;
  pendingReboot: boolean;
  security: string;
  checkedAt: string;
};
export function HealthPage({
  onNavigate,
}: {
  onNavigate: (id: string) => void;
}) {
  const [data, setData] = useState<Health | null>(null),
    [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  const lock = useRef(false);
  async function check() {
    if (lock.current) return;
    lock.current = true;
    setBusy(true);
    setError("");
    try {
      setData(await invoke<Health>("health_check"));
    } catch (e) {
      setError(String(e));
    } finally {
      lock.current = false;
      setBusy(false);
    }
  }
  useEffect(() => {
    if (isTauri()) void check();
  }, []);
  return (
    <section className="surface">
      <div className="section-head">
        <h2>电脑体检</h2>
        <button
          className="secondary-button"
          disabled={busy}
          onClick={() => void check()}
        >
          {busy ? "检查中…" : "重新检查"}
        </button>
      </div>
      {error && <p className="inline-error">{error}</p>}
      {data && (
        <>
          <div className="setting-line">
            <span>内存占用</span>
            <strong>{data.memoryPercent}%</strong>
            <button
              className="ghost-button"
              onClick={() => onNavigate("open_processes")}
            >
              进程管理 →
            </button>
          </div>
          {data.drives.map((d) => (
            <div className="setting-line" key={d.name}>
              <span>{d.name} 可用空间</span>
              <strong>
                {formatBytes(d.freeBytes)} / {formatBytes(d.totalBytes)}
              </strong>
              <button
                className="ghost-button"
                onClick={() => onNavigate("open_storage")}
              >
                检查大文件 →
              </button>
            </div>
          ))}
          <div className="setting-line">
            <span>更新状态</span>
            <strong>
              {data.pendingReboot ? "更新后需要重启" : "无待处理的重启"}
            </strong>
          </div>
          <div className="setting-line">
            <span>Windows 安全</span>
            <strong>{data.security}</strong>
          </div>
          <div className="button-row">
            <button
              className="primary-button"
              onClick={() => onNavigate("open_cleaning")}
            >
              扫描清理
            </button>
            <button
              className="secondary-button"
              onClick={() => onNavigate("open_startup")}
            >
              开机管理
            </button>
            <button
              className="secondary-button"
              onClick={() => onNavigate("open_network")}
            >
              网络检测
            </button>
          </div>
        </>
      )}
      {!data && !busy && !error && (
        <p className="scope-note">点击重新检查读取电脑状态。</p>
      )}
    </section>
  );
}

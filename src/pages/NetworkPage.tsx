import { useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
type Network = {
  adapters: Array<{ name: string; speed: string; description: string }>;
  addresses: Array<{ name: string; address: string }>;
  dnsOk: boolean;
  dnsError: string;
  webOk: boolean;
  webError: string;
  latencyMs: number;
  proxyEnabled: boolean;
  proxyServer: string;
};
export function NetworkPage({
  onOpenTarget,
}: {
  onOpenTarget: (target: string) => void;
}) {
  const [data, setData] = useState<Network | null>(null),
    [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  const lock = useRef(false);
  async function check() {
    if (lock.current) return;
    lock.current = true;
    setBusy(true);
    setError("");
    try {
      setData(await invoke<Network>("network_diagnostics"));
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
        <h2>网络检测</h2>
        <button
          className="secondary-button"
          disabled={busy}
          onClick={() => void check()}
        >
          {busy ? "检测中…" : "重新检测"}
        </button>
      </div>
      {error && <p className="inline-error">{error}</p>}
      {data && (
        <>
          <div className="setting-line">
            <span>DNS 解析</span>
            <strong>{data.dnsOk ? "正常" : "失败"}</strong>
          </div>
          {data.dnsError && <p className="scope-note">{data.dnsError}</p>}
          <div className="setting-line">
            <span>HTTPS 连通性</span>
            <strong>
              {data.webOk ? `正常 · ${data.latencyMs} ms` : "连接失败"}
            </strong>
          </div>
          {data.webError && <p className="scope-note">{data.webError}</p>}
          {data.adapters.map((a) => (
            <div className="setting-line" key={a.name}>
              <span>{a.name}</span>
              <strong>{a.speed}</strong>
            </div>
          ))}
          {data.addresses.map((a, i) => (
            <div className="setting-line" key={i}>
              <span>{a.name}</span>
              <code>{a.address}</code>
            </div>
          ))}
          <div className="setting-line">
            <span>系统代理</span>
            <strong>
              {data.proxyEnabled
                ? data.proxyServer || "已启用"
                : "未启用手动代理"}
            </strong>
          </div>
          <p className="scope-note">
            HTTPS 为连通性检测耗时，网卡速率为链路速率。
          </p>
        </>
      )}
      <div className="button-row">
        <button
          className="secondary-button"
          onClick={() => onOpenTarget("ms-settings:network-status")}
        >
          网络设置 ↗
        </button>
        <button
          className="ghost-button"
          onClick={() => onOpenTarget("ms-settings:network-proxy")}
        >
          代理设置 ↗
        </button>
      </div>
    </section>
  );
}

import { useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ConfirmDialog } from "../ConfirmDialog";
import { ApplicationIcon } from "../ApplicationIcon";

type Tool = {
  id: string;
  title: string;
  description: string;
  admin: boolean;
  restart: boolean;
  disruptive: boolean;
  scopes: string[];
  limitation: string;
};
type Probe = {
  id: string;
  name: string;
  target: string;
  ok: boolean;
  reachable: boolean;
  portal: boolean;
  latencyMs: number;
  detail: string;
};
type Scan = {
  id: string;
  checkedAt: string;
  state: string;
  summary: string;
  issues: { id: string; title: string; detail: string; repairable: boolean }[];
  plan: Tool[];
  probes: Probe[];
  raw: {
    adapters: {
      name: string;
      description: string;
      status: string;
      linkSpeed: string;
      interfaceIndex: number;
      hardwareInterface: boolean;
    }[];
    dns: {
      interfaceAlias: string;
      interfaceIndex: number;
      serverAddresses: string[];
      automatic: boolean;
    }[];
    dnsTests: { name: string; ok: boolean; detail: string }[];
    ips: { interfaceIndex: number; addresses: string[] }[];
    proxy: { enabled: boolean; server: string; autoConfigUrl: string };
    winHttp: { accessType: number; server: string | null } | null;
    proxyProcesses: { name: string; id: number; path: string }[];
    proxyServices: { name: string; status: string }[];
    routes: {
      interfaceAlias: string;
      destinationPrefix: string;
      nextHop: string;
      routeMetric: number;
    }[];
    hostsEntries: string[];
    collectionErrors: string[];
    environmentProxy: {
      name: string;
      configured: boolean;
      machineConfigured: boolean;
    }[];
  };
};
type Backup = {
  id: string;
  createdAt: string;
  title: string;
  canRestore: boolean;
  limitation: string;
};
type Result = {
  success: boolean;
  message: string;
  logs: string[];
  backupId: string | null;
  restartRequired: boolean;
  scan: Scan | null;
  verificationError: string | null;
};
type Pending =
  { kind: "repair"; tool: Tool | null } | { kind: "restore"; backup: Backup };
const time = (value: string) =>
  new Date(value).toLocaleString("zh-CN", { hour12: false });
type Measurement = {
  mbps: number | null;
  latencyMs: number | null;
  jitterMs: number | null;
  bytes: number;
  requestedBytes: number;
  seconds: number;
  completedRequests: number;
  error: string | null;
};
type SpeedResult = {
  latency: Measurement;
  download: Measurement;
  upload: Measurement;
  checkedAt: string;
  server: string;
};

export function NetworkPage({
  onOpenTarget,
}: {
  onOpenTarget: (target: string) => void;
}) {
  const [scan, setScan] = useState<Scan | null>(null);
  const [tools, setTools] = useState<Tool[]>([]);
  const [backups, setBackups] = useState<Backup[]>([]);
  const [busy, setBusy] = useState("");
  const [error, setError] = useState("");
  const [result, setResult] = useState<Result | null>(null);
  const [notice, setNotice] = useState("");
  const [pending, setPending] = useState<Pending | null>(null);
  const [speed, setSpeed] = useState<SpeedResult | null>(null);
  const [speedBusy, setSpeedBusy] = useState(false);
  const [speedStage, setSpeedStage] = useState("");
  const [speedError, setSpeedError] = useState("");
  const [stopping, setStopping] = useState(false);
  const speedRunning = useRef(false);
  const stopRequested = useRef(false);
  const lock = useRef(false);
  async function run(label: string, action: () => Promise<void>) {
    if (lock.current) return;
    lock.current = true;
    setBusy(label);
    setError("");
    setNotice("");
    try {
      await action();
    } catch (e) {
      setError(String(e));
    } finally {
      lock.current = false;
      setBusy("");
    }
  }
  async function check() {
    await run("检查网络中…", async () => {
      setScan(await invoke<Scan>("network_scan"));
      setBackups(await invoke<Backup[]>("network_backups"));
    });
  }
  async function speedtest() {
    if (lock.current) return;
    lock.current = true;
    setSpeedBusy(true);
    speedRunning.current = true;
    stopRequested.current = false;
    setBusy("测速中…");
    setSpeed(null);
    setSpeedError("");
    setSpeedStage("准备测速…");
    let unlisten: (() => void) | undefined;
    try {
      unlisten = await listen<string>("network-speed-stage", ({ payload }) =>
        setSpeedStage(
          {
            latency: "测量延迟…",
            download: "测量下载带宽…",
            upload: "测量上传带宽…",
          }[payload] ?? "测速中…",
        ),
      );
      if (!speedRunning.current) return;
      if (stopRequested.current) throw "测速已停止";
      setSpeed(await invoke<SpeedResult>("network_speedtest"));
    } catch (e) {
      setSpeedError(String(e));
    } finally {
      unlisten?.();
      setSpeedBusy(false);
      speedRunning.current = false;
      setStopping(false);
      setBusy("");
      lock.current = false;
    }
  }
  async function stopSpeedtest() {
    stopRequested.current = true;
    setStopping(true);
    try {
      await invoke("network_cancel_speedtest");
    } catch (e) {
      setSpeedError(String(e));
      setStopping(false);
    }
  }
  useEffect(() => {
    if (!isTauri()) return;
    void invoke<Tool[]>("network_tools")
      .then(setTools)
      .catch((e) => setError(String(e)));
    void check();
    return () => {
      if (speedRunning.current) {
        speedRunning.current = false;
        void invoke("network_cancel_speedtest").catch(() => {});
      }
    };
  }, []);
  async function confirm() {
    const action = pending;
    setPending(null);
    if (!action || !scan) return;
    await run(
      action.kind === "repair" ? "修复并复检中…" : "还原并复检中…",
      async () => {
        const next =
          action.kind === "restore"
            ? await invoke<Result>("network_restore", {
                backupId: action.backup.id,
              })
            : await invoke<Result>("network_repair", {
                scanId: scan.id,
                actionId: action.tool?.id ?? null,
              });
        setResult(next);
        if (next.scan) setScan(next.scan);
        else setScan(null);
        setBackups(await invoke<Backup[]>("network_backups"));
      },
    );
  }
  const dnsOk = scan?.raw.dnsTests.some((t) => t.ok);
  const networkOk = scan?.raw.adapters.some(
    (a) => a.status === "Up" && a.hardwareInterface,
  );
  const internetOk = scan?.probes.some(
    (p) => p.id.startsWith("system-") && p.ok,
  );
  const proxyOk = scan?.raw.proxy.enabled
    ? scan.probes.filter((p) => p.id.startsWith("local-")).some((p) => p.ok)
      ? "端口可用"
      : scan.raw.proxy.autoConfigUrl
        ? "自动代理"
        : "查看详情"
    : scan?.raw.proxy.autoConfigUrl
      ? "自动代理"
      : "未启用";
  const description =
    pending?.kind === "restore"
      ? `还原“${pending.backup.title}”涉及的配置。${pending.backup.limitation} 还原前会再备份当前配置，并申请管理员权限。`
      : pending?.tool
        ? `${pending.tool.description} ${pending.tool.limitation}${pending.tool.admin ? " 需要管理员授权。" : ""}${pending.tool.restart ? " 完成后需要重启电脑。" : ""} 操作前自动备份，完成后重新检查。`
        : `将执行：${scan?.plan.map((t) => t.title).join("、") ?? ""}。先备份，修复后自动检查。${scan?.plan.some((t) => t.admin) ? " 部分操作需要管理员授权。" : ""}${scan?.plan.some((t) => t.disruptive) ? " 网络可能短暂中断。" : ""}`;
  return (
    <div className="page-stack network-page" aria-busy={Boolean(busy)}>
      <section className="surface network-summary">
        <div className="network-summary__heading">
          <span
            className={`network-symbol ${scan?.state === "healthy" ? "network-symbol--ok" : ""}`}
            aria-hidden="true"
          >
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.6"
            >
              <path d="M3 9a14 14 0 0 1 18 0M6 12a9 9 0 0 1 12 0M9 15a4 4 0 0 1 6 0" />
              <circle cx="12" cy="19" r=".8" />
            </svg>
          </span>
          <div>
            <h2>{busy || scan?.summary || "检查网络连接"}</h2>
            <p className="scope-note">
              {scan
                ? `上次检查 ${time(scan.checkedAt)}`
                : "网卡、DNS、代理与联网状态"}
            </p>
          </div>
        </div>
        <div className="network-checks">
          {[
            ["网络连接", networkOk ? "已连接" : "未连接"],
            ["DNS 解析", dnsOk ? "可用" : "未通过"],
            ["联网验证", internetOk ? "可用" : "未通过"],
            ["系统代理", proxyOk],
          ].map(([label, status]) => (
            <div key={label}>
              <small>{label}</small>
              <strong>{scan ? status : "—"}</strong>
            </div>
          ))}
        </div>
        <div className="button-row">
          <button
            className={
              scan?.plan.length ? "secondary-button" : "primary-button"
            }
            disabled={Boolean(busy)}
            onClick={() => void check()}
          >
            {scan ? "重新检查" : "检查网络"}
          </button>
          <button
            className="primary-button"
            disabled={Boolean(busy) || !scan?.plan.length}
            onClick={() => setPending({ kind: "repair", tool: null })}
          >
            修复问题{scan?.plan.length ? ` · ${scan.plan.length} 项` : ""}
          </button>
          {scan && (
            <button
              className="ghost-button"
              disabled={Boolean(busy)}
              onClick={() =>
                void run("导出报告中…", async () => {
                  const path = await invoke<string>("network_export_report", {
                    scanId: scan.id,
                  });
                  setNotice(`报告已保存：${path}`);
                })
              }
            >
              导出报告
            </button>
          )}
        </div>
        {error && (
          <p className="inline-error" role="alert">
            {error}
          </p>
        )}
        {notice && (
          <p className="scope-note" role="status">
            {notice}
          </p>
        )}
        {result && (
          <div
            className={
              result.success
                ? "network-result"
                : "network-result network-result--error"
            }
            role="status"
          >
            <strong>{result.message}</strong>
            {result.verificationError && <p>{result.verificationError}</p>}
            {result.logs.length > 0 && (
              <details>
                <summary>操作记录</summary>
                {result.logs.map((line, i) => (
                  <p key={i}>{line}</p>
                ))}
              </details>
            )}
          </div>
        )}
      </section>
      <section className="surface network-speed">
        <div className="network-speed__heading">
          <h2>带宽测速</h2>
          {speedBusy ? (
            <button
              className="secondary-button"
              disabled={stopping}
              onClick={() => void stopSpeedtest()}
            >
              {stopping ? "停止中…" : "停止测速"}
            </button>
          ) : (
            <button
              className="secondary-button"
              disabled={Boolean(busy) || !isTauri()}
              onClick={() => void speedtest()}
            >
              {speed ? "重新测速" : "开始测速"}
            </button>
          )}
        </div>
        <div className="network-speed__metrics">
          {(
            [
              ["下载", speed?.download.mbps, "Mbps"],
              ["上传", speed?.upload.mbps, "Mbps"],
              ["延迟", speed?.latency.latencyMs, "ms"],
              ["抖动", speed?.latency.jitterMs, "ms"],
            ] as const
          ).map(([label, value, unit]) => (
            <div key={label}>
              <small>{label}</small>
              <strong>
                {value == null ? "—" : value.toFixed(1)} <span>{unit}</span>
              </strong>
            </div>
          ))}
        </div>
        {(speedBusy || speed) && (
          <p className="scope-note" role="status">
            {speedBusy
              ? speedStage
              : speed
                ? `${time(speed.checkedAt)} · ${speed.download.error || speed.upload.error || speed.latency.error ? "部分完成" : "已完成"} · 测试请求 ${((speed.download.requestedBytes + speed.upload.requestedBytes) / 1024 / 1024).toFixed(1)} MiB`
                : ""}
          </p>
        )}
        <p className="scope-note">
          测当前连接到 Cloudflare 的速率 · 沿用当前代理 · 最多 144 MiB 生成数据
          · 服务器可见连接 IP
        </p>
        {speedError && (
          <p className="inline-error" role="alert">
            {speedError}
          </p>
        )}
        {speed &&
          (
            [
              ["下载", speed.download],
              ["上传", speed.upload],
              ["延迟", speed.latency],
            ] as const
          ).map(
            ([label, measurement]) =>
              measurement.error && (
                <p className="inline-error" role="alert" key={label}>
                  {label}测试未完成：{measurement.error}
                </p>
              ),
          )}
      </section>
      {scan && scan.issues.length > 0 && (
        <section className="surface network-issues">
          <h2>检查结果</h2>
          {scan.issues.map((issue) => (
            <div className="network-issue" key={issue.id}>
              <span
                className={`network-issue__dot ${issue.repairable ? "network-issue__dot--repair" : ""}`}
                aria-hidden="true"
              />
              <div>
                <strong>{issue.title}</strong>
                <p className="scope-note">{issue.detail}</p>
              </div>
              {issue.repairable && <small>可修复</small>}
            </div>
          ))}
        </section>
      )}
      {scan && (
        <details className="surface network-details">
          <summary>检测详情</summary>
          <div className="network-detail-grid">
            <div>
              <h3>联网测试</h3>
              {scan.probes.map((p) => (
                <div className="network-detail-row" key={p.id}>
                  <div>
                    <strong>{p.name}</strong>
                    <small title={p.detail}>
                      {p.ok ? `${p.latencyMs} ms` : p.detail}
                    </small>
                  </div>
                  <span>
                    {p.ok
                      ? "通过"
                      : p.portal
                        ? "需登录"
                        : p.reachable
                          ? "站点限制"
                          : "未通过"}
                  </span>
                </div>
              ))}
              <p className="scope-note">
                测试代表该连接路径，不等于下载测速或所有应用均可联网。
              </p>
            </div>
            <div>
              <h3>网络配置</h3>
              {scan.raw.adapters
                .filter((a) => a.status === "Up")
                .map((a) => (
                  <div className="network-detail-row" key={a.interfaceIndex}>
                    <div>
                      <strong>{a.name}</strong>
                      <small>{a.description}</small>
                      <small>
                        {scan.raw.ips
                          .find((ip) => ip.interfaceIndex === a.interfaceIndex)
                          ?.addresses.join(" · ")}
                      </small>
                      <small>
                        {scan.raw.dns
                          .filter((d) => d.interfaceIndex === a.interfaceIndex)
                          .map(
                            (d) =>
                              `${d.automatic ? "自动" : "手动"} DNS：${d.serverAddresses.join("、") || "无"}`,
                          )
                          .join(" / ")}
                      </small>
                    </div>
                    <span>{a.linkSpeed}</span>
                  </div>
                ))}
              <div className="network-detail-row">
                <div>
                  <strong>用户代理</strong>
                  <small>
                    {scan.raw.proxy.enabled
                      ? scan.raw.proxy.server
                      : "未启用手动代理"}
                    {scan.raw.proxy.autoConfigUrl
                      ? " · 配置了自动代理脚本"
                      : ""}
                  </small>
                </div>
              </div>
              <div className="network-detail-row">
                <div>
                  <strong>WinHTTP 后台代理</strong>
                  <small>
                    {scan.raw.winHttp?.server || "直连或未配置静态代理"}
                  </small>
                </div>
              </div>
              <div className="network-detail-row">
                <div>
                  <strong>命令行代理变量</strong>
                  <small>
                    {scan.raw.environmentProxy
                      .filter((p) => p.configured || p.machineConfigured)
                      .map((p) => p.name)
                      .join("、") || "未配置用户或机器级变量"}
                  </small>
                </div>
              </div>
              <div className="network-detail-row">
                <div>
                  <strong>Hosts 自定义记录</strong>
                  <small>
                    {scan.raw.hostsEntries.length} 条 · 保留现有内容
                  </small>
                </div>
              </div>
            </div>
          </div>
          {scan.raw.proxyProcesses.length > 0 && (
            <div className="network-proxy-apps">
              <h3>代理与隧道程序</h3>
              {scan.raw.proxyProcesses.map((p) => (
                <div key={p.id}>
                  <ApplicationIcon target={p.path} />
                  <strong>{p.name}</strong>
                  <small>PID {p.id}</small>
                </div>
              ))}
            </div>
          )}
          <details className="network-route-details">
            <summary>路由与服务</summary>
            {scan.raw.routes.map((r, i) => (
              <p className="scope-note" key={i}>
                {r.interfaceAlias} · {r.destinationPrefix} → {r.nextHop} · 跃点{" "}
                {r.routeMetric}
              </p>
            ))}
            {scan.raw.proxyServices.map((s) => (
              <p className="scope-note" key={s.name}>
                {s.name} · {s.status}
              </p>
            ))}
          </details>
          {scan.raw.collectionErrors.map((e, i) => (
            <p className="inline-error" key={i}>
              {e}
            </p>
          ))}
        </details>
      )}
      <details className="surface network-details">
        <summary>专项修复</summary>
        <p className="scope-note">
          自动修复无效时使用。会影响连接的操作均需再次确认。
        </p>
        <div className="network-tool-grid">
          {tools.map((tool) => (
            <article key={tool.id}>
              <div>
                <h3>{tool.title}</h3>
                <p className="scope-note">{tool.description}</p>
              </div>
              <button
                className="secondary-button"
                disabled={Boolean(busy) || !scan}
                onClick={() => setPending({ kind: "repair", tool })}
              >
                执行{tool.restart ? " · 需重启" : ""}
              </button>
            </article>
          ))}
        </div>
      </details>
      <details className="surface network-details">
        <summary>
          备份与还原{backups.length ? ` · ${backups.length}` : ""}
        </summary>
        <div className="section-head">
          <p className="scope-note">
            按本次操作还原配置，不能回滚网卡重装和网络栈重置。
          </p>
          <button
            className="secondary-button"
            disabled={Boolean(busy)}
            onClick={() =>
              void run("备份网络中…", async () => {
                await invoke("network_backup");
                setBackups(await invoke<Backup[]>("network_backups"));
                setNotice("网络配置已备份。");
              })
            }
          >
            备份当前配置
          </button>
        </div>
        {backups.length ? (
          backups.map((backup) => (
            <div className="network-backup-row" key={backup.id}>
              <div>
                <strong title={backup.title}>{backup.title}</strong>
                <small>{time(backup.createdAt)}</small>
                <small>{backup.limitation}</small>
              </div>
              <button
                className="secondary-button"
                disabled={Boolean(busy) || !backup.canRestore || !scan}
                onClick={() => setPending({ kind: "restore", backup })}
              >
                {backup.canRestore ? "还原" : "仅记录"}
              </button>
            </div>
          ))
        ) : (
          <p className="scope-note">暂无备份。修复前会自动创建。</p>
        )}
      </details>
      <div className="button-row">
        <button
          className="ghost-button"
          disabled={Boolean(busy)}
          onClick={() => onOpenTarget("ms-settings:network-status")}
        >
          Windows 网络设置 ↗
        </button>
        <button
          className="ghost-button"
          disabled={Boolean(busy)}
          onClick={() => onOpenTarget("ms-settings:network-proxy")}
        >
          代理设置 ↗
        </button>
      </div>
      <ConfirmDialog
        open={Boolean(pending)}
        title={
          pending?.kind === "restore"
            ? "还原网络配置"
            : pending?.tool?.title || "修复网络问题"
        }
        description={description}
        confirmLabel={pending?.kind === "restore" ? "备份并还原" : "备份并修复"}
        onConfirm={() => void confirm()}
        onCancel={() => setPending(null)}
      />
    </div>
  );
}

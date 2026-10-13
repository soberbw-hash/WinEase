import { useEffect, useRef, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { ConfirmDialog } from "./ConfirmDialog";
export function AppUpdater() {
  const [state, setState] = useState<
    "idle" | "checking" | "downloading" | "ready" | "installing" | "error"
  >("idle");
  const [error, setError] = useState(""),
    [version, setVersion] = useState(""),
    [prompt, setPrompt] = useState(false);
  const task = useRef(false),
    update = useRef<Update | null>(null);
  async function checkUpdate(manual = false) {
    if (task.current) return;
    if (update.current) {
      if (manual) setPrompt(true);
      return;
    }
    task.current = true;
    setError("");
    setState("checking");
    let candidate: Update | null = null;
    try {
      candidate = await check({ timeout: 15000 });
      if (!candidate) {
        setState("idle");
        if (manual) setError("当前已是最新版本。");
        return;
      }
      // Signed manifests must keep the download in this project's GitHub release assets.
      const raw = candidate.rawJson;
      const platforms = raw.platforms as
        Record<string, { url?: string }> | undefined;
      const url = raw.url ?? platforms?.["windows-x86_64"]?.url;
      if (
        typeof url !== "string" ||
        !url.startsWith(
          "https://github.com/soberbw-hash/WinEase/releases/download/",
        )
      )
        throw Error("更新下载地址不属于 WinEase 官方发布。");
      setVersion(candidate.version);
      setState("downloading");
      await candidate.download(undefined, { timeout: 120000 });
      update.current = candidate;
      candidate = null;
      setState("ready");
      setPrompt(true);
    } catch (e) {
      setState("error");
      setError(manual ? `更新检查失败：${String(e)}` : "");
    } finally {
      await candidate?.close().catch(() => {});
      task.current = false;
    }
  }
  async function install() {
    if (!update.current || task.current) return;
    task.current = true;
    setState("installing");
    setPrompt(false);
    try {
      await update.current.install({ restartAfterInstall: true });
    } catch (e) {
      setError(`安装失败：${String(e)}`);
      setState("ready");
    } finally {
      task.current = false;
    }
  }
  useEffect(() => {
    if (isTauri()) void checkUpdate();
    const onCheck = () => void checkUpdate(true);
    window.addEventListener("winease-check-update", onCheck);
    return () => window.removeEventListener("winease-check-update", onCheck);
  }, []);
  return (
    <>
      <div className="app-update-status" aria-live="polite">
        {state === "ready" && (
          <button className="secondary-button" onClick={() => setPrompt(true)}>
            新版 {version} 已下载 · 安装并重启
          </button>
        )}
        {(state === "checking" || state === "downloading") && (
          <span>{state === "checking" ? "检查更新…" : "后台下载更新…"}</span>
        )}
        {state === "installing" && <span>正在启动安装程序…</span>}
        {error && <span role="status">{error}</span>}
      </div>
      <ConfirmDialog
        open={prompt}
        title={`WinEase ${version} 已下载`}
        description="安装包已通过签名校验。安装完成后将重新打开 WinEase，请先保存其他工作。"
        confirmLabel="安装并重启"
        onConfirm={() => void install()}
        onCancel={() => setPrompt(false)}
      />
    </>
  );
}

import { useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { ConfirmDialog } from "../ConfirmDialog";
import { formatBytes } from "../format";
import type { CleaningScan, ToolActionResult } from "../types";

export function CleaningPage({
  onResult,
}: {
  onResult: (result: ToolActionResult) => void;
}) {
  const [scan, setScan] = useState<CleaningScan | null>(null);
  const [selected, setSelected] = useState<string[]>([]);
  const [busy, setBusy] = useState<"scan" | "clean" | null>(null);
  const [error, setError] = useState("");
  const [result, setResult] = useState<ToolActionResult | null>(null);
  const [confirm, setConfirm] = useState(false);
  const lock = useRef(false);
  useEffect(() => {
    if (isTauri()) void scanFiles();
  }, []);
  const bytes =
    scan?.categories
      .filter((item) => selected.includes(item.id))
      .reduce((sum, item) => sum + item.sizeBytes, 0) ?? 0;
  async function scanFiles() {
    if (lock.current) return;
    lock.current = true;
    setBusy("scan");
    setError("");
    setScan(null);
    setResult(null);
    setSelected([]);
    try {
      const next = await invoke<CleaningScan>("scan_cleaning");
      setScan(next);
      setSelected(
        next.categories
          .filter((item) => item.fileCount > 0)
          .map((item) => item.id),
      );
    } catch (err) {
      setError(String(err));
    } finally {
      lock.current = false;
      setBusy(null);
    }
  }
  async function clean() {
    setConfirm(false);
    if (!scan || !selected.length || lock.current) return;
    lock.current = true;
    setBusy("clean");
    setError("");
    try {
      const next = await invoke<ToolActionResult>("clean_selected", {
        scanId: scan.scanId,
        categoryIds: selected,
      });
      setResult(next);
      onResult(next);
    } catch (err) {
      setError(String(err));
    } finally {
      setScan(null);
      setSelected([]);
      lock.current = false;
      setBusy(null);
    }
  }
  return (
    <div className="page-stack">
      <section className="surface">
        <div className="section-head">
          <h2>深度清理</h2>
          <button
            className="secondary-button"
            disabled={busy !== null}
            onClick={() => void scanFiles()}
          >
            {busy === "scan" ? "扫描中…" : scan ? "重新扫描" : "开始扫描"}
          </button>
        </div>
        <p className="scope-note">
          网页、应用与图形缓存保留最近 24 小时；临时文件与错误报告保留最近 7
          天。保留登录信息与个人文件。
        </p>
        {error && (
          <p className="inline-error" role="alert">
            {error}
          </p>
        )}
        {scan ? (
          <>
            <div className="cleaning-list">
              {scan.categories.map((item) => (
                <label className="setting-line" key={item.id}>
                  <span>
                    <input
                      type="checkbox"
                      checked={selected.includes(item.id)}
                      disabled={busy !== null || item.fileCount === 0}
                      onChange={(event) =>
                        setSelected((current) =>
                          event.target.checked
                            ? [...current, item.id]
                            : current.filter((id) => id !== item.id),
                        )
                      }
                    />
                    {item.label}
                  </span>
                  <span className="cleaning-list__size">
                    {item.fileCount} 个文件 · {formatBytes(item.sizeBytes)}
                  </span>
                </label>
              ))}
            </div>
            <div className="cleaning-summary">
              <div>
                <span>预计可释放</span>
                <strong>{formatBytes(bytes)}</strong>
              </div>
              <button
                className="primary-button"
                disabled={!selected.length || busy !== null}
                onClick={() => setConfirm(true)}
              >
                清理所选
              </button>
            </div>
            <p className="scope-note">
              已跳过 {scan.skippedEntries} 个近期、不可读或链接项目
            </p>
          </>
        ) : (
          !result && (
            <div className="empty-state">
              {busy === "clean"
                ? "清理中…"
                : busy === "scan"
                  ? "正在统计文件"
                  : "扫描后选择清理项目"}
            </div>
          )
        )}
      </section>
      {result && (
        <section className="surface" aria-live="polite">
          <div className="section-head">
            <h2>清理结果</h2>
            <span
              className={`pill ${result.success ? "pill--success" : "pill--warning"}`}
            >
              {result.success ? "完成" : "部分未完成"}
            </span>
          </div>
          <p>{result.summary}</p>
          <pre className="result-details">{result.details}</pre>
        </section>
      )}
      <ConfirmDialog
        open={confirm}
        title="清理所选项目？"
        description={`预计释放 ${formatBytes(bytes)}。文件将直接删除，无法撤销。`}
        confirmLabel="清理"
        onCancel={() => setConfirm(false)}
        onConfirm={() => void clean()}
      />
    </div>
  );
}

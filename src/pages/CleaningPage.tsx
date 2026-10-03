import { useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { ConfirmDialog } from "../ConfirmDialog";
import { formatBytes } from "../format";

import type { CleaningGroup, CleaningScan, ToolActionResult } from "../types";

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
  const [expanded, setExpanded] = useState<string[]>([]);
  const [details, setDetails] = useState<
    Record<
      string,
      {
        files: Array<{
          id: number;
          name: string;
          path: string;
          sizeBytes: number;
        }>;
        total: number;
      }
    >
  >({});
  const [detailError, setDetailError] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState<string[]>([]);
  const detailGeneration = useRef(0);
  async function loadDetails(item: CleaningGroup, offset = 0) {
    if (!scan || loading.includes(item.id)) return;
    const generation = detailGeneration.current;
    setLoading((ids) => [...ids, item.id]);
    setDetailError((errors) => ({ ...errors, [item.id]: "" }));
    try {
      const next = await invoke<{
        files: Array<{
          id: number;
          name: string;
          path: string;
          sizeBytes: number;
        }>;
        total: number;
      }>("cleaning_files", { scanId: scan.scanId, groupId: item.id, offset });
      if (generation === detailGeneration.current)
        setDetails((current) => ({
          ...current,
          [item.id]: {
            total: next.total,
            files: offset
              ? [...(current[item.id]?.files ?? []), ...next.files]
              : next.files,
          },
        }));
    } catch (err) {
      if (generation === detailGeneration.current)
        setDetailError((errors) => ({ ...errors, [item.id]: String(err) }));
    } finally {
      if (generation === detailGeneration.current)
        setLoading((ids) => ids.filter((id) => id !== item.id));
    }
  }
  function toggleDetails(item: CleaningGroup) {
    setExpanded((ids) =>
      ids.includes(item.id)
        ? ids.filter((id) => id !== item.id)
        : [...ids, item.id],
    );
    if (!details[item.id]) void loadDetails(item);
  }
  function toggleGroup(id: string, checked: boolean) {
    setSelected((ids) =>
      checked
        ? [...new Set([...ids, id])]
        : ids.filter((value) => value !== id),
    );
  }
  function renderGroups(items: CleaningGroup[], title: string) {
    if (!items.length) return null;
    const all = items.every((item) => selected.includes(item.id));
    const subtotal = items
      .filter((item) => selected.includes(item.id))
      .reduce((n, item) => n + item.sizeBytes, 0);
    return (
      <section className="cleanup-group">
        <div className="cleanup-group__head">
          <label>
            <input
              type="checkbox"
              checked={all}
              disabled={busy !== null}
              aria-label={`选择${title}`}
              onChange={(e) =>
                setSelected((ids) =>
                  e.target.checked
                    ? [...new Set([...ids, ...items.map((item) => item.id)])]
                    : ids.filter((id) => !items.some((item) => item.id === id)),
                )
              }
            />
            {title}
          </label>
          <span>
            {formatBytes(subtotal)} /{" "}
            {formatBytes(items.reduce((n, item) => n + item.sizeBytes, 0))}
          </span>
        </div>
        {items.map((item) => (
          <div className="cleanup-item" key={item.id}>
            <div className="cleanup-item__head">
              <input
                type="checkbox"
                aria-label={`选择 ${item.label}`}
                checked={selected.includes(item.id)}
                disabled={busy !== null}
                onChange={(e) => toggleGroup(item.id, e.target.checked)}
              />
              <button
                className="cleanup-expand"
                aria-expanded={expanded.includes(item.id)}
                aria-controls={`files-${item.id}`}
                onClick={() => toggleDetails(item)}
              >
                <span aria-hidden="true">
                  {expanded.includes(item.id) ? "⌄" : "›"}
                </span>
                <strong>{item.label}</strong>
                <small>{item.fileCount} 个文件</small>
              </button>
              <span className="tabular">{formatBytes(item.sizeBytes)}</span>
            </div>
            {expanded.includes(item.id) && (
              <div className="cleanup-files" id={`files-${item.id}`}>
                <p className="scope-note" title={item.path}>
                  {item.path}
                </p>
                {detailError[item.id] && (
                  <p role="alert" className="inline-error">
                    {detailError[item.id]}{" "}
                    <button
                      className="ghost-button"
                      onClick={() => void loadDetails(item)}
                    >
                      重试
                    </button>
                  </p>
                )}
                {details[item.id]?.files.map((file) => (
                  <div className="cleanup-file" key={file.id}>
                    <div>
                      <strong title={file.name}>{file.name}</strong>
                      <small title={file.path}>{file.path}</small>
                    </div>
                    <span>{formatBytes(file.sizeBytes)}</span>
                    <button
                      className="ghost-button"
                      onClick={() =>
                        void invoke("open_target", {
                          target: file.path.substring(
                            0,
                            file.path.lastIndexOf("\\"),
                          ),
                        }).catch((err) => setError(String(err)))
                      }
                    >
                      目录 ↗
                    </button>
                  </div>
                ))}
                {loading.includes(item.id) ? (
                  <p role="status">读取明细…</p>
                ) : (
                  details[item.id] &&
                  details[item.id].files.length < details[item.id].total && (
                    <button
                      className="secondary-button"
                      onClick={() =>
                        void loadDetails(item, details[item.id].files.length)
                      }
                    >
                      查看更多（{details[item.id].files.length}/
                      {details[item.id].total}）
                    </button>
                  )
                )}
              </div>
            )}
          </div>
        ))}
      </section>
    );
  }
  useEffect(() => {
    if (isTauri()) void scanFiles();
  }, []);
  const bytes =
    scan?.groups
      .filter((item) => selected.includes(item.id))
      .reduce((sum, item) => sum + item.sizeBytes, 0) ?? 0;
  async function scanFiles() {
    if (lock.current) return;
    lock.current = true;
    setBusy("scan");
    setError("");
    detailGeneration.current++;
    setExpanded([]);
    setDetails({});
    setDetailError({});
    setLoading([]);
    setScan(null);
    setResult(null);
    setSelected([]);
    try {
      const next = await invoke<CleaningScan>("scan_cleaning");
      setScan(next);
      setSelected(
        next.groups
          .filter((item) => item.recommended && item.fileCount > 0)
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
            <div className="cleanup-discovery">
              <div>
                <strong>
                  发现{" "}
                  {formatBytes(
                    scan.groups.reduce((n, item) => n + item.sizeBytes, 0),
                  )}{" "}
                  可清理项目
                </strong>
                <span>已选 {formatBytes(bytes)} · 点击项目查看文件</span>
              </div>

              <button
                className="primary-button"
                disabled={!selected.length || busy !== null}
                onClick={() => setConfirm(true)}
              >
                清理所选
              </button>
            </div>
            {renderGroups(
              scan.groups.filter(
                (item) =>
                  item.recommended &&
                  ["temporary", "logs"].includes(item.category),
              ),
              "推荐系统清理",
            )}
            {renderGroups(
              scan.groups.filter(
                (item) =>
                  item.recommended &&
                  !["temporary", "logs"].includes(item.category),
              ),
              "推荐应用清理",
            )}
            {renderGroups(
              scan.groups.filter((item) => !item.recommended),
              "其他可选项目",
            )}
            {scan.groups.length === 0 && (
              <div className="empty-state">未发现符合保留期限的缓存文件</div>
            )}
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
        <div className="cleanup-system-entry">
          <span>Windows 更新、传递优化及回收站</span>
          <button
            className="ghost-button"
            onClick={() =>
              void invoke("open_target", {
                target: "ms-settings:storagesense",
              }).catch((err) => setError(String(err)))
            }
          >
            系统清理 ↗
          </button>
        </div>
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

import { useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { ConfirmDialog } from "../ConfirmDialog";
import { formatBytes } from "../format";

import { ApplicationIcon } from "../ApplicationIcon";
import { buildCleaningTree, type CleaningNode } from "../cleaningTree";
import type { CleaningScan, ToolActionResult } from "../types";

function SelectionCheck({
  ids,
  selected,
  disabled,
  label,
  onChange,
}: {
  ids: string[];
  selected: string[];
  disabled: boolean;
  label: string;
  onChange: (checked: boolean) => void;
}) {
  const ref = useRef<HTMLInputElement>(null);
  const count = ids.filter((id) => selected.includes(id)).length;
  useEffect(() => {
    if (ref.current)
      ref.current.indeterminate = count > 0 && count < ids.length;
  }, [count, ids.length]);
  return (
    <input
      ref={ref}
      type="checkbox"
      aria-label={label}
      checked={count === ids.length}
      disabled={disabled}
      onChange={(e) => onChange(e.target.checked)}
    />
  );
}
function CacheIcon({ name }: { name: string }) {
  return (
    <span className="cleanup-symbol" aria-hidden="true">
      <svg
        viewBox="0 0 24 24"
        width="24"
        height="24"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.6"
      >
        {name === "Windows" ? (
          <path d="M3 3h8v8H3zM14 3h7v8h-7zM3 14h8v7H3zM14 14h7v7h-7z" />
        ) : name.includes("图形") || name === "NVIDIA" ? (
          <>
            <rect x="5" y="5" width="14" height="14" rx="2" />
            <path d="M9 9h6v6H9zM8 2v3M16 2v3M8 19v3M16 19v3M2 8h3M2 16h3M19 8h3M19 16h3" />
          </>
        ) : name.includes("日志") || name.includes("报告") ? (
          <>
            <path d="M5 3h10l4 4v14H5zM15 3v5h4M8 12h8M8 16h6" />
          </>
        ) : (
          <>
            <ellipse cx="12" cy="5" rx="8" ry="3" />
            <path d="M4 5v7c0 4 16 4 16 0V5M4 12v6c0 4 16 4 16 0v-6" />
          </>
        )}
      </svg>
    </span>
  );
}

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
  async function loadDetails(item: CleaningNode, offset = 0) {
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
      }>("cleaning_files", {
        scanId: scan.scanId,
        groupId: item.ids[0],
        groupIds: item.ids,
        offset,
      });
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
  function toggleDetails(item: CleaningNode) {
    setExpanded((ids) =>
      ids.includes(item.id)
        ? ids.filter((id) => id !== item.id)
        : [...ids, item.id],
    );
    if (!details[item.id]) void loadDetails(item);
  }
  function toggleIds(ids: string[], checked: boolean) {
    setSelected((current) =>
      checked
        ? [...new Set([...current, ...ids])]
        : current.filter((id) => !ids.includes(id)),
    );
  }
  function renderNode(item: CleaningNode, software: boolean) {
    const open = expanded.includes(item.id);
    return (
      <div
        className={`cleanup-item ${software ? "cleanup-software" : "cleanup-kind"}`}
        key={item.id}
      >
        <div className="cleanup-item__head">
          <SelectionCheck
            ids={item.ids}
            selected={selected}
            disabled={busy !== null}
            label={`选择 ${item.label}`}
            onChange={(checked) => toggleIds(item.ids, checked)}
          />
          <button
            className="cleanup-expand"
            aria-expanded={open}
            aria-controls={`files-${item.id}`}
            onClick={() => {
              if (software)
                setExpanded((ids) =>
                  ids.includes(item.id)
                    ? ids.filter((id) => id !== item.id)
                    : [...ids, item.id],
                );
              else toggleDetails(item);
            }}
          >
            <span className="cleanup-chevron" aria-hidden="true">
              {open ? "⌄" : "›"}
            </span>
            {software && item.iconTarget ? (
              <ApplicationIcon target={item.iconTarget} />
            ) : (
              <CacheIcon name={item.label} />
            )}
            <strong>{item.label}</strong>
            <small>{item.fileCount} 个文件</small>
            {item.groups.every((g) => !g.recommended) && (
              <small className="cleanup-optional">可选</small>
            )}
          </button>
          <span className="tabular">{formatBytes(item.sizeBytes)}</span>
        </div>
        {software ? (
          open && (
            <div className="cleanup-kinds" id={`files-${item.id}`}>
              {item.children.map((child) => renderNode(child, false))}
            </div>
          )
        ) : (
          <>
            {expanded.includes(item.id) && (
              <div className="cleanup-files" id={`files-${item.id}`}>
                <p
                  className="scope-note"
                  title={item.groups.map((g) => g.path).join("\n")}
                >
                  {item.groups.length === 1
                    ? item.groups[0].path
                    : `${item.groups.length} 个缓存目录 · 已合并全部文件`}
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
          </>
        )}
      </div>
    );
  }
  function renderGroups(items: CleaningNode[], title: string) {
    if (!items.length) return null;
    const ids = items.flatMap((item) => item.ids);
    const subtotal = (scan?.groups ?? [])
      .filter((item) => ids.includes(item.id) && selected.includes(item.id))
      .reduce((sum, item) => sum + item.sizeBytes, 0);
    return (
      <section className="cleanup-group">
        <div className="cleanup-group__head">
          <label>
            <SelectionCheck
              ids={ids}
              selected={selected}
              disabled={busy !== null}
              label={`选择${title}`}
              onChange={(checked) => toggleIds(ids, checked)}
            />
            {title}
          </label>
          <span>
            {formatBytes(subtotal)} /{" "}
            {formatBytes(items.reduce((sum, item) => sum + item.sizeBytes, 0))}
          </span>
        </div>
        {items.map((item) => renderNode(item, true))}
      </section>
    );
  }
  useEffect(() => {
    if (isTauri()) void scanFiles();
  }, []);
  const tree = buildCleaningTree(scan?.groups ?? []);
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
              tree.filter((item) => item.label === "Windows"),
              "系统清理",
            )}
            {renderGroups(
              tree.filter((item) => item.label !== "Windows"),
              "应用清理",
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

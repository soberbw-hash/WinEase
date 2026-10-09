import { useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { formatBytes } from "../format";
import { ConfirmDialog } from "../ConfirmDialog";
import { DiskMap, type UsageNode } from "../DiskMap";
import { ApplicationIcon } from "../ApplicationIcon";
type Row = {
  id: number;
  name: string;
  path: string;
  sizeBytes: number;
  group: number | null;
  canRecycle: boolean;
  modifiedAt: number;
};
type Scan = {
  scanId: string;
  files: Row[];
  skipped: number;
  limited: boolean;
  usage: UsageNode[];
  totalBytes: number;
  breakdown: Array<{ id: string; label: string; sizeBytes: number }>;
};
type Drive = { path: string; totalBytes: number; freeBytes: number };
type View = "overview" | "large" | "duplicates";
const types = [
  ["all", "所有", "▤"],
  ["documents", "文档", "▱"],
  ["images", "图片", "▧"],
  ["archives", "压缩包", "▣"],
  ["videos", "视频", "▻"],
  ["audio", "音频", "♫"],
  ["other", "其他", "◇"],
];
export function fileKind(name: string) {
  const ext = name.toLowerCase().split(".").pop() ?? "";
  if (
    [
      "doc",
      "docx",
      "xls",
      "xlsx",
      "ppt",
      "pptx",
      "pdf",
      "txt",
      "md",
      "csv",
      "rtf",
      "odt",
    ].includes(ext)
  )
    return "documents";
  if (
    [
      "jpg",
      "jpeg",
      "png",
      "gif",
      "webp",
      "bmp",
      "svg",
      "heic",
      "tif",
      "tiff",
      "raw",
    ].includes(ext)
  )
    return "images";
  if (["zip", "rar", "7z", "tar", "gz", "bz2", "xz", "iso"].includes(ext))
    return "archives";
  if (["mp4", "mkv", "avi", "mov", "wmv", "webm", "flv", "m4v"].includes(ext))
    return "videos";
  if (["mp3", "wav", "flac", "aac", "ogg", "m4a", "wma", "opus"].includes(ext))
    return "audio";
  return "other";
}
function fileTypeBadge(name: string) {
  const extension = name.includes(".") ? name.split(".").pop()!.toLowerCase() : "";
  if (["safetensors", "gguf", "onnx", "pt", "pth"].includes(extension)) return "模型";
  return extension ? extension.slice(0, 5).toUpperCase() : "文件";
}
function fileTypeLabel(name: string) {
  const extension = name.includes(".") ? name.split(".").pop()!.toLowerCase() : "";
  return fileTypeBadge(name) === "模型" ? `AI 模型文件（.${extension}）` : extension ? `.${extension} 文件` : "文件";
}
export function FilesPage({
  initialDuplicates,
  navigationRequest = 0,
  onOpenTarget,
}: {
  initialDuplicates: boolean;
  navigationRequest?: number;
  onOpenTarget: (path: string) => void;
}) {
  const [view, setView] = useState<View>(
    initialDuplicates ? "duplicates" : "overview",
  );
  const [drives, setDrives] = useState<Drive[]>([]),
    [root, setRoot] = useState("");
  const [tasks, setTasks] = useState<Record<string, { scan?: Scan; busy?: "scan" | "recycle" | null; error?: string; message?: string }>>({});
  const [selected, setSelected] = useState<number[]>([]);
  const [confirm, setConfirm] = useState(false);
  const [kind, setKind] = useState("all"),
    [minimum, setMinimum] = useState(10 * 1024 * 1024),
    [search, setSearch] = useState(""),
    [page, setPage] = useState(0);
  const cacheKey = (drive: string, duplicates: boolean) => `${drive.toLowerCase()}:${duplicates}`;
  const scanDuplicates = view === "duplicates";
  const key = cacheKey(root, scanDuplicates);
  const current = tasks[key] ?? {};
  const scan = current.scan ?? null, busy = current.busy ?? null;
  const error = current.error ?? "", message = current.message ?? "";
  const locks = useRef(new Set<string>()), mounted = useRef(true);
  function updateTask(taskKey: string, patch: Partial<typeof current>) {
    setTasks(items => ({...items, [taskKey]: {...items[taskKey], ...patch}}));
  }
  useEffect(() => {
    setView(initialDuplicates ? "duplicates" : navigationRequest ? "large" : "overview");
    setSelected([]);
    setConfirm(false);
  }, [initialDuplicates, navigationRequest]);
  useEffect(() => {
    mounted.current = true;
    if (isTauri())
      void invoke<Drive[]>("storage_drives")
        .then((items) => {
          if (mounted.current) {
            setDrives(items);
            setRoot(
              items.find((d) => d.path.toLowerCase() === "c:\\")?.path ??
                items[0]?.path ??
                "",
            );
          }
        })
        .catch(() => {});
    return () => {
      mounted.current = false;
      if (locks.current.size) void invoke("cancel_file_scan");
    };
  }, []);
  async function start() {
    // One volume-statistics task and one duplicate task may run together.
    const lane = String(scanDuplicates);
    if (locks.current.has(lane) || !root) return;
    locks.current.add(lane);
    const taskKey = key;
    updateTask(taskKey, {busy: "scan", error: "", message: ""});
    setSelected([]);
    setPage(0);
    try {
      const next = await invoke<Scan>("scan_personal_files", {root, duplicates: scanDuplicates});
      if (mounted.current) {
        updateTask(taskKey, {scan: next});
        void invoke<Drive[]>("storage_drives").then(setDrives).catch(() => {});
      }
    } catch (e) {
      if (mounted.current) updateTask(taskKey, {error: String(e)});
    } finally {
      locks.current.delete(lane);
      if (mounted.current) updateTask(taskKey, {busy: null});
    }
  }
  function changeView(next: View) {
    setSelected([]);
    setConfirm(false);
    setView(next);
    setPage(0);
    setSearch("");
    setKind("all");
  }
  async function recycle() {
    setConfirm(false);
    if (!scan || busy || locks.current.has("recycle")) return;
    locks.current.add("recycle");
    const taskKey = key;
    updateTask(taskKey, {busy: "recycle", error: ""});
    try {
      const result = await invoke<string>("recycle_selected_files", {scanId: scan.scanId, ids: selected});
      updateTask(taskKey, {message: result, scan: undefined});
    } catch (e) {
      updateTask(taskKey, {error: String(e)});
    } finally {
      updateTask(taskKey, {busy: null});
      setSelected([]);
      locks.current.delete("recycle");
    }
  }
  const groups = new Map<number, Row[]>();
  for (const file of scan?.files ?? [])
    if (file.group !== null)
      groups.set(file.group, [...(groups.get(file.group) ?? []), file]);
  function copies(files: Row[]) {
    return [...files]
      .sort(
        (a, b) =>
          b.modifiedAt - a.modifiedAt ||
          a.path.length - b.path.length ||
          a.path.localeCompare(b.path),
      )
      .slice(1)
      .filter((f) => f.canRecycle)
      .map((f) => f.id);
  }
  function toggle(file: Row, checked: boolean) {
    setSelected((ids) =>
      checked
        ? [...new Set([...ids, file.id])]
        : ids.filter((id) => id !== file.id),
    );
  }
  const bytes = (scan?.files ?? [])
    .filter((f) => selected.includes(f.id))
    .reduce((n, f) => n + f.sizeBytes, 0);
  const entireGroup = [...groups.values()].some((files) =>
    files.every((f) => selected.includes(f.id)),
  );
  const filter = (f: Row) =>
    (kind === "all" || fileKind(f.name) === kind) &&
    (!search ||
      `${f.name} ${f.path}`.toLowerCase().includes(search.toLowerCase()));
  const visible = (scan?.files ?? []).filter(
    (f) => filter(f) && (view === "duplicates" || f.sizeBytes >= minimum),
  );
  const visibleGroups = [...groups.entries()]
    .filter(([, files]) => files.some(filter))
    .sort(
      ([, a], [, b]) =>
        b[0].sizeBytes * (b.length - 1) - a[0].sizeBytes * (a.length - 1),
    );
  const drive = drives.find((d) => d.path === root);
  function fileRow(file: Row) {
    return (
      <div className="storage-file-row" key={file.id}>
        <input
          type="checkbox"
          aria-label={`选择 ${file.name} ${file.path}`}
          checked={selected.includes(file.id)}
          disabled={busy !== null || !file.canRecycle}
          onChange={(e) => toggle(file, e.target.checked)}
        />
        <span className="storage-file-icon" title={fileTypeLabel(file.name)}>
          <ApplicationIcon target={file.path} file fallback={
            <svg viewBox="0 0 24 24" width="28" height="28" fill="none" stroke="currentColor" strokeWidth="1.5"><path d="M5 2h9l5 5v15H5zM14 2v6h5M8 13h8M8 17h6" /></svg>
          } />
          <small>{fileTypeBadge(file.name)}</small>
        </span>
        <strong title={file.name}>
          {file.name}
          {!file.canRecycle && <small> · 只读</small>}
        </strong>
        <span className="file-date">
          {file.modifiedAt
            ? new Date(file.modifiedAt * 1000).toLocaleDateString()
            : "—"}
        </span>
        <span className="tabular">{formatBytes(file.sizeBytes)}</span>
        <button
          className="file-path"
          title={file.path}
          onClick={() =>
            onOpenTarget(file.path.substring(0, file.path.lastIndexOf("\\")))
          }
        >
          {file.path}
        </button>
      </div>
    );
  }
  return (
    <div className="page-stack">
      <nav className="subnav" aria-label="空间管理">
        <button
          disabled={busy === "recycle"}
          aria-pressed={view === "overview"}
          onClick={() => changeView("overview")}
        >
          ▥ 存储概览{tasks[cacheKey(root, false)]?.busy === "scan" ? " · 扫描中" : ""}
        </button>
        <button
          disabled={busy === "recycle"}
          aria-pressed={view === "large"}
          onClick={() => changeView("large")}
        >
          ▤ 大文件{tasks[cacheKey(root, false)]?.busy === "scan" ? " · 扫描中" : ""}
        </button>
        <button
          disabled={busy === "recycle"}
          aria-pressed={view === "duplicates"}
          onClick={() => changeView("duplicates")}
        >
          ▣ 重复文件{tasks[cacheKey(root, true)]?.busy === "scan" ? " · 扫描中" : ""}
        </button>
      </nav>
      <section className="surface storage-surface">
        <div className="section-head">
          <div>
            <h2>
              {view === "overview"
                ? "存储概览"
                : view === "large"
                  ? "查找大文件"
                  : "重复文件"}
            </h2>
            {scan && (
              <p className="scope-note">
                {view === "duplicates"
                  ? `${groups.size} 组重复文件 · 多余副本 ${formatBytes([...groups.values()].reduce((n, files) => n + files[0].sizeBytes * (files.length - 1), 0))}`
                  : `已统计 ${formatBytes(scan.totalBytes)} 文件`}
              </p>
            )}
          </div>
          <div className="button-row">
            <select
              aria-label="扫描磁盘"
              value={root}
              disabled={busy !== null}
              onChange={(e) => {
                setRoot(e.target.value);
                setSelected([]);
              }}
            >
              {drives.map((d) => (
                <option key={d.path} value={d.path}>
                  {d.path}
                </option>
              ))}
            </select>
            <button
              className="primary-button"
              disabled={busy === "recycle" || !root || (busy !== "scan" && Object.entries(tasks).some(([id, task]) => id.endsWith(`:${scanDuplicates}`) && task.busy === "scan"))}
              onClick={() =>
                busy === "scan" ? void invoke("cancel_file_scan", {duplicates: scanDuplicates}) : void start()
              }
            >
              {busy === "scan" ? "停止扫描" : scan ? "重新扫描" : "开始扫描"}
            </button>
          </div>
        </div>
        {error && (
          <p role="alert" className="inline-error">
            {error}
          </p>
        )}
        {message && <p role="status">{message}</p>}
        {drive && (
          <div className="storage-drive">
            <div>
              <strong>本地磁盘（{root.slice(0, 2)}）</strong>
              <span>
                已用 {formatBytes(drive.totalBytes - drive.freeBytes)} /{" "}
                {formatBytes(drive.totalBytes)} · 可用{" "}
                {formatBytes(drive.freeBytes)}
              </span>
            </div>
            <div
              className="storage-capacity"
              role="meter"
              aria-label="磁盘已用空间"
              aria-valuenow={Math.round(
                (100 * (drive.totalBytes - drive.freeBytes)) /
                  Math.max(1, drive.totalBytes),
              )}
              aria-valuemin={0}
              aria-valuemax={100}
            >
              <span
                style={{
                  width: `${(100 * (drive.totalBytes - drive.freeBytes)) / Math.max(1, drive.totalBytes)}%`,
                }}
              />
            </div>
          </div>
        )}
        {busy === "scan" && (
          <div className="empty-state" role="status">
            {view === "duplicates"
              ? "正在逐个校验文件内容…"
              : "正在统计磁盘文件…"}
          </div>
        )}
        {scan && view === "overview" && (
          <>
            <div className="storage-breakdown">
              {scan.breakdown.map((item) => (
                <div key={item.id}>
                  <strong>{item.label}</strong>
                  <span>{formatBytes(item.sizeBytes)}</span>
                  <small>
                    {scan.totalBytes
                      ? ((item.sizeBytes * 100) / scan.totalBytes).toFixed(1)
                      : 0}
                    % 已扫描文件
                  </small>
                </div>
              ))}
            </div>
            <p className="scope-note">
              分类按路径统计已扫描文件；磁盘已用空间包含受保护和未读取的内容，两者可能不同。
            </p>
            <DiskMap
              key={scan.scanId}
              root={root}
              usage={scan.usage}
              files={scan.files}
              total={scan.totalBytes}
              onOpen={onOpenTarget}
            />
            <div className="storage-shortcuts">
              <button
                className="secondary-button"
                onClick={() => changeView("large")}
              >
                查看大文件
              </button>
              <button
                className="secondary-button"
                onClick={() => changeView("duplicates")}
              >
                查找重复文件
              </button>
            </div>
          </>
        )}
        {scan && view !== "overview" && (
          <>
            <div className="storage-filters">
              <div className="file-type-tabs" aria-label="文件类型">
                {types.map(([id, label, icon]) => (
                  <button
                    key={id}
                    aria-pressed={kind === id}
                    onClick={() => {
                      setKind(id);
                      setPage(0);
                    }}
                  >
                    <span aria-hidden="true">{icon}</span> {label}
                  </button>
                ))}
              </div>
              {view === "large" && (
                <label>
                  大小{" "}
                  <select
                    aria-label="最小文件大小"
                    value={minimum}
                    onChange={(e) => {
                      setMinimum(Number(e.target.value));
                      setPage(0);
                    }}
                  >
                    {[10, 100, 500, 1024].map((m) => (
                      <option key={m} value={m * 1024 * 1024}>
                        {m === 1024 ? ">1 GB" : `>${m} MB`}
                      </option>
                    ))}
                  </select>
                </label>
              )}
            </div>
            <input
              className="manager-search"
              aria-label="搜索文件"
              placeholder="搜索文件名或路径"
              value={search}
              onChange={(e) => {
                setSearch(e.target.value);
                setPage(0);
              }}
            />
            <div className="storage-selection">
              <span>
                已选 {selected.length} 个 · {formatBytes(bytes)}
              </span>
              <div className="button-row">
                {view === "duplicates" && (
                  <button
                    className="ghost-button"
                    disabled={busy !== null}
                    onClick={() =>
                      setSelected(
                        visibleGroups
                          .flatMap(([, files]) => copies(files))
                          .slice(0, 2000),
                      )
                    }
                  >
                    智能勾选
                  </button>
                )}
                <button
                  className="secondary-button"
                  disabled={
                    busy !== null ||
                    !selected.length ||
                    entireGroup ||
                    selected.length > 2000
                  }
                  onClick={() => setConfirm(true)}
                >
                  移到回收站
                </button>
              </div>
            </div>
            {entireGroup && (
              <p role="alert" className="inline-error">
                每组至少保留一个文件。
              </p>
            )}
            {selected.length > 2000 && (
              <p className="inline-error">每次最多处理 2000 个文件。</p>
            )}
            <div className="storage-file-header">
              <span>名称</span>
              <span>修改日期</span>
              <span>大小 ↓</span>
              <span>路径</span>
            </div>
            {view === "large"
              ? visible.slice(page * 100, (page + 1) * 100).map(fileRow)
              : visibleGroups
                  .slice(page * 30, (page + 1) * 30)
                  .map(([id, files]) => (
                    <details className="duplicate-group" key={id} open>
                      <summary>
                        <span aria-hidden="true">⌄</span>
                        <ApplicationIcon target={files[0].path} file />
                        <strong>{files[0].name}</strong>
                        <span>
                          {files.length} 份 · 可移除{" "}
                          {formatBytes(files[0].sizeBytes * (files.length - 1))}
                        </span>
                        <button
                          className="ghost-button"
                          disabled={busy !== null}
                          onClick={(e) => {
                            e.preventDefault();
                            const ids = copies(files);
                            setSelected((current) =>
                              ids.every((id) => current.includes(id))
                                ? current.filter((id) => !ids.includes(id))
                                : [...new Set([...current, ...ids])],
                            );
                          }}
                        >
                          选择多余副本
                        </button>
                      </summary>
                      {files.map(fileRow)}
                    </details>
                  ))}
            {(view === "large" ? visible.length : visibleGroups.length) ===
              0 && <div className="empty-state">没有符合筛选条件的文件</div>}
            <div className="storage-pagination">
              <button
                className="ghost-button"
                disabled={page === 0}
                onClick={() => setPage((p) => p - 1)}
              >
                上一页
              </button>
              <span>
                第 {page + 1} 页 ·{" "}
                {view === "large"
                  ? `${visible.length} 个文件`
                  : `${visibleGroups.length} 组`}
              </span>
              <button
                className="ghost-button"
                disabled={
                  (page + 1) * (view === "large" ? 100 : 30) >=
                  (view === "large" ? visible.length : visibleGroups.length)
                }
                onClick={() => setPage((p) => p + 1)}
              >
                下一页
              </button>
            </div>
            <p className="scope-note">
              文件大小不代表垃圾。系统与应用目录只读；移入回收站后可恢复，清空回收站才释放空间。智能勾选保留每组修改时间最新的一份。
            </p>
          </>
        )}
        {scan?.limited && (
          <p className="scope-note">
            当前结果可能未覆盖全盘；大文件列表最多显示 2000
            个文件，扫描受时间和数量上限限制。占用图及分类仅代表已扫描数据。
          </p>
        )}
        {scan && (
          <small className="scope-note">
            跳过无法读取的项目：{scan.skipped}
          </small>
        )}
        {!scan && !busy && !message && (
          <div className="empty-state">选择磁盘，点击开始扫描</div>
        )}
      </section>
      <ConfirmDialog
        open={confirm}
        title={`将 ${selected.length} 个文件移到回收站？`}
        description={`合计 ${formatBytes(bytes)}。请确认文件不再需要；重复组至少保留一份。`}
        confirmLabel="移到回收站"
        onCancel={() => setConfirm(false)}
        onConfirm={() => void recycle()}
      />
    </div>
  );
}

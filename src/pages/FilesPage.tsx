import { useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { formatBytes } from "../format";
import { ConfirmDialog } from "../ConfirmDialog";
type Row = {
  id: number;
  name: string;
  path: string;
  sizeBytes: number;
  group: number | null;
};
type Scan = { scanId: string; files: Row[]; skipped: number; limited: boolean };
export function FilesPage({
  initialDuplicates,
  onOpenTarget,
}: {
  initialDuplicates: boolean;
  onOpenTarget: (path: string) => void;
}) {
  const [duplicates, setDuplicates] = useState(initialDuplicates),
    [folders, setFolders] = useState<string[]>([]),
    [root, setRoot] = useState("");
  const [scan, setScan] = useState<Scan | null>(null),
    [selected, setSelected] = useState<number[]>([]),
    [busy, setBusy] = useState<"scan" | "recycle" | null>(null),
    [error, setError] = useState(""),
    [message, setMessage] = useState(""),
    [confirm, setConfirm] = useState(false);
  const lock = useRef(false),
    mounted = useRef(true);
  useEffect(() => {
    setDuplicates(initialDuplicates);
    setScan(null);
    setSelected([]);
  }, [initialDuplicates]);
  useEffect(() => {
    mounted.current = true;
    if (isTauri())
      void invoke<string[]>("personal_folders")
        .then((items) => {
          if (mounted.current) {
            setFolders(items);
            setRoot(items[items.length - 1] ?? "");
          }
        })
        .catch((e) => setError(String(e)));
    return () => {
      mounted.current = false;
      if (lock.current) void invoke("cancel_file_scan");
    };
  }, []);
  async function start() {
    if (lock.current || !root) return;
    lock.current = true;
    setBusy("scan");
    setError("");
    setMessage("");
    setScan(null);
    setSelected([]);
    try {
      const next = await invoke<Scan>("scan_personal_files", {
        root,
        duplicates,
      });
      if (mounted.current) setScan(next);
    } catch (e) {
      if (mounted.current) setError(String(e));
    } finally {
      lock.current = false;
      if (mounted.current) setBusy(null);
    }
  }
  async function recycle() {
    setConfirm(false);
    if (!scan || lock.current) return;
    lock.current = true;
    setBusy("recycle");
    setError("");
    try {
      setMessage(
        await invoke<string>("recycle_selected_files", {
          scanId: scan.scanId,
          ids: selected,
        }),
      );
    } catch (e) {
      setError(String(e));
    } finally {
      setScan(null);
      setSelected([]);
      setBusy(null);
      lock.current = false;
    }
  }
  function selectCopies() {
    if (!scan) return;
    const seen = new Set<number>();
    setSelected(
      scan.files
        .filter((f) => {
          if (f.group === null) return false;
          if (seen.has(f.group)) return true;
          seen.add(f.group);
          return false;
        })
        .map((f) => f.id),
    );
  }
  const size =
    scan?.files
      .filter((f) => selected.includes(f.id))
      .reduce((n, f) => n + f.sizeBytes, 0) ?? 0;
  const allGroupSelected =
    duplicates &&
    scan?.files.some(
      (f) =>
        selected.includes(f.id) &&
        !scan.files.some(
          (other) => other.group === f.group && !selected.includes(other.id),
        ),
    );
  return (
    <div className="page-stack">
      <nav className="subnav" aria-label="空间扫描分类">
        <button
          disabled={busy !== null}
          aria-pressed={!duplicates}
          onClick={() => {
            setDuplicates(false);
            setScan(null);
            setSelected([]);
          }}
        >
          大文件
        </button>
        <button
          disabled={busy !== null}
          aria-pressed={duplicates}
          onClick={() => {
            setDuplicates(true);
            setScan(null);
            setSelected([]);
          }}
        >
          重复文件
        </button>
      </nav>
      <section className="surface">
        <div className="section-head">
          <h2>{duplicates ? "重复文件检测" : "大文件检测"}</h2>
          <div className="button-row">
            {busy === "scan" ? (
              <button
                className="secondary-button"
                onClick={() => void invoke("cancel_file_scan")}
              >
                取消扫描
              </button>
            ) : (
              <button
                className="primary-button"
                disabled={busy !== null || !root}
                onClick={() => void start()}
              >
                开始扫描
              </button>
            )}
          </div>
        </div>
        <label className="folder-field">
          <span>扫描目录</span>
          <input
            list="personal-folders"
            value={root}
            disabled={busy !== null}
            onChange={(e) => setRoot(e.target.value)}
            placeholder="选择或输入个人文件目录"
          />
          <datalist id="personal-folders">
            {folders.map((p) => (
              <option key={p} value={p} />
            ))}
          </datalist>
        </label>
        <p className="scope-note">
          {duplicates
            ? "按文件内容校验，每组至少保留一个。"
            : "列出 100 MB 以上的个人文件。"}{" "}
          文件移入回收站，可恢复；清空后才释放空间。
        </p>
        {error && (
          <p className="inline-error" role="alert">
            {error}
          </p>
        )}
        {message && <p role="status">{message}</p>}
        {busy === "scan" && (
          <p className="scope-note" role="status">
            正在扫描，重复文件需要逐个校验内容…
          </p>
        )}
        {scan && (
          <>
            <div className="setting-line">
              <span>
                {scan.files.length} 个文件 · 已选 {selected.length} 个 /{" "}
                {formatBytes(size)}
              </span>
              <div className="button-row">
                {duplicates && (
                  <button
                    className="ghost-button"
                    disabled={busy !== null}
                    onClick={selectCopies}
                  >
                    选择多余副本
                  </button>
                )}
                <button
                  className="secondary-button"
                  disabled={
                    busy !== null ||
                    !selected.length ||
                    Boolean(allGroupSelected)
                  }
                  onClick={() => setConfirm(true)}
                >
                  移到回收站
                </button>
              </div>
            </div>
            {allGroupSelected && (
              <p className="inline-error">每组至少保留一个文件。</p>
            )}
            {scan.limited && (
              <p className="scope-note">
                扫描达到数量或时间上限，请选择更小的目录继续扫描。
              </p>
            )}
            <div className="manager-list">
              {scan.files.map((f) => (
                <div className="manager-row" key={f.id}>
                  <input
                    type="checkbox"
                    aria-label={`选择 ${f.name}`}
                    checked={selected.includes(f.id)}
                    disabled={busy !== null}
                    onChange={(e) =>
                      setSelected((ids) =>
                        e.target.checked
                          ? [...ids, f.id]
                          : ids.filter((id) => id !== f.id),
                      )
                    }
                  />
                  <div className="manager-row__text">
                    <strong>
                      {f.group !== null ? `组 ${f.group + 1} · ` : ""}
                      {f.name}
                    </strong>
                    <small title={f.path}>{f.path}</small>
                  </div>
                  <span className="tabular">{formatBytes(f.sizeBytes)}</span>
                  <button
                    className="ghost-button"
                    onClick={() =>
                      onOpenTarget(
                        f.path.substring(0, f.path.lastIndexOf("\\")),
                      )
                    }
                  >
                    目录 ↗
                  </button>
                </div>
              ))}
            </div>
            {scan.files.length === 0 && (
              <div className="empty-state">
                未发现{duplicates ? "重复文件" : "100 MB 以上文件"}
              </div>
            )}
            <small>跳过无法读取的项目：{scan.skipped}</small>
          </>
        )}
      </section>
      <ConfirmDialog
        open={confirm}
        title={`将 ${selected.length} 个文件移到回收站？`}
        description={`文件合计 ${formatBytes(size)}。可从 Windows 回收站恢复。`}
        confirmLabel="移到回收站"
        onCancel={() => setConfirm(false)}
        onConfirm={() => void recycle()}
      />
    </div>
  );
}

import { formatDuration, formatMemory, formatRelativeTime } from "./format";
import * as Dialog from "@radix-ui/react-dialog";
import { useRef } from "react";
import type { SystemSnapshot, ToolActionResult } from "./types";

type InfoDrawerProps = {
  open: boolean;
  snapshot: SystemSnapshot | null;
  history: ToolActionResult[];
  onClose: () => void;
  onOpenTarget: (target: string) => void;
};

export function InfoDrawer({
  open,
  snapshot,
  history,
  onClose,
  onOpenTarget,
}: InfoDrawerProps) {
  const origin = useRef<HTMLElement | null>(null);

  return (
    <Dialog.Root
      open={open}
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content
          className="drawer-panel"
          aria-describedby={undefined}
          onOpenAutoFocus={() => {
            origin.current = document.activeElement as HTMLElement;
          }}
          onCloseAutoFocus={(event) => {
            event.preventDefault();
            origin.current?.focus();
          }}
        >
          <div className="drawer-panel__header">
            <div>
              <Dialog.Title>设备与记录</Dialog.Title>
            </div>
            <button className="ghost-button" type="button" onClick={onClose}>
              关闭
            </button>
          </div>

          <section className="soft-card drawer-section">
            <h3>当前机器</h3>
            <dl className="drawer-meta">
              <div>
                <dt>系统</dt>
                <dd>
                  {snapshot
                    ? `${snapshot.osName} ${snapshot.osBuild}`
                    : "正在读取"}
                </dd>
              </div>
              <div>
                <dt>处理器</dt>
                <dd>{snapshot?.cpuName ?? "正在读取"}</dd>
              </div>
              <div>
                <dt>内存</dt>
                <dd>
                  {snapshot
                    ? `${formatMemory(snapshot.memoryUsedMb)} / ${formatMemory(snapshot.memoryTotalMb)}`
                    : "正在读取"}
                </dd>
              </div>
              <div>
                <dt>刷新时间</dt>
                <dd>{formatRelativeTime(snapshot?.collectedAt)}</dd>
              </div>
            </dl>
          </section>

          <section className="soft-card drawer-section">
            <h3>操作记录</h3>

            <div className="history-list">
              {history.length === 0 ? (
                <div className="empty-state">还没有操作记录。</div>
              ) : (
                history.map((item) => (
                  <article
                    key={`${item.actionId}-${item.durationMs}-${item.title}`}
                    className="history-item"
                  >
                    <div className="history-item__top">
                      <strong>{item.title}</strong>
                      <span
                        className={`pill ${item.success ? "pill--success" : "pill--warning"}`}
                      >
                        {item.success ? "成功" : "注意"}
                      </span>
                    </div>
                    <p>{item.summary}</p>
                    <small>耗时 {formatDuration(item.durationMs)}</small>
                    <details>
                      <summary>详情</summary>
                      <pre>{item.details}</pre>
                      {item.warnings.map((warning) => (
                        <p key={warning}>{warning}</p>
                      ))}
                    </details>
                    {item.outputPath ? (
                      <button
                        className="ghost-button"
                        type="button"
                        onClick={() => onOpenTarget(item.outputPath!)}
                      >
                        打开位置
                      </button>
                    ) : null}
                  </article>
                ))
              )}
            </div>
          </section>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

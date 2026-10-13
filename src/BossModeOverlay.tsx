import type { BossModeViewState } from "./types";

type BossModeOverlayProps = {
  state: BossModeViewState;
};

function LoadingDots() {
  return (
    <div className="boss-mode__dots" aria-hidden="true">
      {Array.from({ length: 5 }).map((_, index) => (
        <span key={index} style={{ animationDelay: `${index * 120}ms` }} />
      ))}
    </div>
  );
}

export function BossModeOverlay({ state }: BossModeOverlayProps) {
  if (state.isRebooting) {
    return <div className="boss-mode boss-mode--reboot" />;
  }

  return (
    <div
      className="boss-mode"
      onContextMenu={(event) => event.preventDefault()}
    >
      <div className="boss-mode__content">
        <LoadingDots />
        <p>正在进行更新 {state.percent}%</p>
        <p>请保持计算机打开。</p>
      </div>

      <div className="boss-mode__footer">
        <p>计算机可能会重启几次。</p>
      </div>
    </div>
  );
}

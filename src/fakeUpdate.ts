import type { BossModeViewState } from "./types";
export function getBossModeViewState(elapsedMs: number): BossModeViewState {
  return {
    stageTitle: "正在进行更新",
    stageHint: "",
    percent: Math.min(99, Math.floor(7 + Math.max(0, elapsedMs) / 60000)),
    phaseLabel: "",
    phaseIndex: 0,
    isRebooting: false,
    instruction: "请保持计算机打开。",
  };
}

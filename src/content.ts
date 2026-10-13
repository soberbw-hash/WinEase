import type { SectionId } from "./types";
export const bossModeShortcut = "Ctrl + Alt + B";
export const sections: Array<{ id: SectionId; label: string }> = [
  { id: "home", label: "一键优化" },
  { id: "cleaning", label: "清理" },
  { id: "efficiency", label: "空间管理" },
  { id: "applications", label: "应用管理" },
  { id: "network", label: "网络" },
  { id: "system", label: "系统工具" },
  { id: "components", label: "组件" },
  { id: "settings", label: "设置" },
];
export const windowsToolGroups = [
  { title: "修复与系统管理", icon: "health", items: [
    { label: "手动磁盘优化", detail: "手动整理或 TRIM", target: "open_drive_optimizer" },
    { label: "默认应用", detail: "浏览器与文件关联，也可重置", target: "ms-settings:defaultapps" },
    { label: "设备与驱动", detail: "查看异常设备", target: "open_devices" },
    { label: "可靠性记录", detail: "查看崩溃与故障时间", target: "open_reliability" },
    { label: "Windows 更新", detail: "系统与可选驱动更新", target: "ms-settings:windowsupdate" },
    { label: "系统恢复", detail: "恢复选项与重置电脑", target: "ms-settings:recovery" },
  ] },
];
export const quickPathTargets: Array<{
  id: string;
  label: string;
  pathKey: "downloads" | "desktop" | "documents";
}> = [
  { id: "downloads", label: "下载", pathKey: "downloads" },
  { id: "desktop", label: "桌面", pathKey: "desktop" },
  { id: "documents", label: "文档", pathKey: "documents" },
];

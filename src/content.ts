import type { SectionId } from "./types";
export const bossModeShortcut = "Ctrl + Alt + B";
export const sections: Array<{ id: SectionId; label: string }> = [
  { id: "home", label: "首页" },
  { id: "cleaning", label: "清理" },
  { id: "efficiency", label: "空间管理" },
  { id: "applications", label: "应用管理" },
  { id: "system", label: "系统工具" },
  { id: "components", label: "组件" },
  { id: "settings", label: "设置" },
];
export const windowsSettingLinks = [
  { label: "存储设置", target: "ms-settings:storagesense" },
  { label: "设置默认浏览器", target: "ms-settings:defaultapps" },
  { label: "重置默认应用", target: "ms-settings:defaultapps" },
  { label: "任务栏设置", target: "ms-settings:taskbar" },
  { label: "Windows 更新", target: "ms-settings:windowsupdate" },
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

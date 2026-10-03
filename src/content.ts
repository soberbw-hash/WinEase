import type { HomeQuickAction, SectionId, ToolDefinition } from "./types";
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
export const homeQuickActions: HomeQuickAction[] = [
  {
    id: "open_cleaning",
    title: "深度清理",
    description: "扫描临时文件与缓存",
    tone: "primary",
  },
  {
    id: "open_health",
    title: "电脑体检",
    description: "检查空间、内存与系统状态",
  },
  { id: "open_storage", title: "大文件", description: "查找占用空间的文件" },
  { id: "open_startup", title: "开机管理", description: "管理自动启动应用" },
];
export const systemTools: ToolDefinition[] = [
  {
    id: "dism_check_health",
    title: "系统快速检查",
    description: "DISM CheckHealth · 需管理员权限",
    tag: "检查",
    note: "",
  },
  {
    id: "dism_scan_health",
    title: "系统深度扫描",
    description: "DISM ScanHealth · 需管理员权限",
    tag: "扫描",
    note: "",
  },
  {
    id: "export_drivers",
    title: "备份驱动",
    description: "导出到文档目录 · 需管理员权限",
    tag: "备份",
    note: "",
  },
];
export const windowsSettingLinks = [
  { label: "存储设置", target: "ms-settings:storagesense" },
  { label: "设置默认浏览器", target: "ms-settings:defaultapps" },
  { label: "重置默认应用", target: "ms-settings:defaultapps" },
  { label: "任务栏设置", target: "ms-settings:taskbar" },
  { label: "Windows 更新", target: "ms-settings:windowsupdate" },
  { label: "默认应用", target: "ms-settings:defaultapps" },
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

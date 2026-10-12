import type { SectionId } from "./types";
export const bossModeShortcut = "Ctrl + Alt + B";
export const sections: Array<{ id: SectionId; label: string }> = [
  { id: "home", label: "一键优化" },
  { id: "cleaning", label: "清理" },
  { id: "efficiency", label: "空间管理" },
  { id: "applications", label: "应用管理" },
  { id: "system", label: "系统工具" },
  { id: "components", label: "组件" },
  { id: "settings", label: "设置" },
];
export const windowsToolGroups = [
  { title: "性能与存储", icon: "files", items: [
    { label: "自动清理", detail: "设置存储感知", target: "ms-settings:storagepolicies" },
    { label: "磁盘优化", detail: "由 Windows 区分 SSD 与机械硬盘", target: "open_drive_optimizer" },
    { label: "电源与睡眠", detail: "耗电与性能设置", target: "ms-settings:powersleep" },
    { label: "游戏模式", detail: "游戏运行偏好", target: "ms-settings:gaming-gamemode" },
    { label: "后台录制", detail: "检查游戏录像占用", target: "ms-settings:gaming-gamedvr" },
  ] },
  { title: "通知与隐私", icon: "health", items: [
    { label: "通知与免打扰", detail: "管理应用通知", target: "ms-settings:notifications" },
    { label: "隐私与推荐内容", detail: "广告与建议偏好", target: "ms-settings:privacy-general" },
    { label: "诊断与反馈", detail: "诊断数据设置", target: "ms-settings:privacy-feedback" },
    { label: "传递优化", detail: "更新的下载与上传带宽", target: "ms-settings:delivery-optimization" },
  ] },
  { title: "修复与系统管理", icon: "health", items: [
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

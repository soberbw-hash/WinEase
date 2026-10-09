# WinEase 品牌资源

`icon-master.png` 是用户提供的原始 PNG（1254 × 1254，RGBA），按原始字节保存。它是应用品牌图标的唯一来源，不重绘、不改色、不裁切、不添加效果。

SHA-256：`92979f558c078217d6bae22aa7a4ee01c73e24204109bb47b19aaa8e80724655`

安装依赖后，从项目根目录运行：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/generate-brand-assets.ps1
```

脚本校验原图，用 Tauri 官方图标工具生成 `src-tauri/icons/` 下的多尺寸 PNG、ICO 和各平台资源，再更新界面图标 `public/brand-icon.png` 和 favicon。透明背景保留；原图不参与修改。

Windows 安装包也包含原图与本说明，位于安装目录的 `branding/` 文件夹，便于以后直接取用。

应用显示名称、窗口标题、安装包与项目名称统一为 **WinEase**。第三方组件继续使用各自官方图标。

为兼容已有用户配置，Tauri 应用标识 `com.sober.win.toolbox`、前端存储键 `win-toolbox:settings:v3_2`、`WinToolbox` 数据目录和注册表备份位置保留。进程保护同时识别新旧 EXE 名称。

# WinEase 更新

## 应用更新

应用启动后在后台请求 `https://github.com/soberbw-hash/WinEase/releases/latest/download/latest.json`，仅接受更高版本；GitHub 失败时不打断启动，可在设置→关于手动重试。使用 Tauri 官方 updater、内置公钥及 Minisign 签名验证，沿用系统代理。下载地址限定此仓库的 release assets。后台下载完成并校验后提示“安装并重启”，用户确认前不启动安装器。Windows 安装器使用 passive 模式并在完成后重新打开应用；机器级安装仍可能需要 Windows 授权。暂不跨应用启动保存下载任务。

当前 GitHub 最新发布仍是旧版 v3.4.0，没有本版 `latest.json`；因此更新协议已接入，但正式发布签名清单前不会收到本版更新。未把缺少清单说成“已是最新版本”。

## 签名与发布准备

私钥不入库、不放在安装包里。当前私钥另存于 Documents/WinEase 项目备份/更新签名密钥，以当前用户及 SYSTEM 的 Windows ACL 保护。公钥已固定到 tauri.conf.json；请备份该私钥，后续版本不能随意换公钥，否则已安装客户端无法验证升级。

运行 `powershell -ExecutionPolicy Bypass -File scripts/build-signed-release.ps1`，或通过 `-SigningKeyPath` 指定私钥。该脚本只在进程环境短暂加载私钥，生成签名 NSIS、`.sig`、`latest.json` 和 SHA256SUMS，写入忽略目录 output/release，不上传。

正式发布时，将 output/release 中的四个文件上传到同版本 `v版本号` 的 GitHub Release，再核验 latest/download/latest.json 中的版本、签名及安装包 URL，并通过旧安装版完成一次升级验收。发布行为需单独授权；构建与模拟检查不代表已经完成真实升级。

官方依据：[Tauri Updater](https://v2.tauri.app/plugin/updater/)。

## 组件更新

支持 winget 的已安装组件显示更新按钮。按钮执行固定包 ID 的 `winget upgrade`，不会通过 repair/force 重新安装。启动 30 秒后及每 6 小时查询 `winget list --upgrade-available`，发现新版时在按钮标注；后台只检查，不自动安装第三方软件。检查失败保留已有状态，解析只接受精确包 ID，过长而被 winget 截断的 ID 可能无法标注，手动更新仍按完整固定 ID 执行。

组件任务按软件独立记录，组件页和设置页仅禁用正在操作的软件，其他软件仍可打开或提交安装、更新等任务。前后端都拒绝同一软件的重复操作；完成或失败只释放本软件的任务状态。后台更新检查不与用户更新共用全局锁。不同安装器是否能真正同时安装由 winget/Windows 安装服务决定，若安装器报告忙碌，展示该任务的失败结果，不冻结其他组件。浏览器模拟验证多个更新及安装同时提交、其他软件打开、页面切换、失败与乱序完成；没有在用户设备安装或更新软件来验证并行安装器行为。

官方依据：[WinGet list](https://learn.microsoft.com/en-us/windows/package-manager/winget/list)。

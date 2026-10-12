# 界面收敛与共享组件

2026-10-13，延续 Windows 清理、调优与常用管理的定位。

## 本轮实现

- 空间图不再只画当前目录的一级文件夹。自动展开占用较大的目录和 Users/用户/Documents 这种单子目录链，最多 240 个块；面积按已扫描的逻辑大小分配，不重复计算父子目录。模型、视频、压缩包、图片、程序和音频使用不同颜色。小块隐藏放不下的文字，悬停保留名称、大小与路径；目录可以下钻，文件可打开所在目录。未进入文件列表的占用显示为对应目录的其他文件。扫描上限与部分结果边界仍见 [清理与空间分析](cleanup-and-storage.md)。
- 优化设置默认只显示居中的标题和箭头。展开时短暂淡入并自动滚到面板；尊重系统减少动画设置。宽窗口一行两项，小窗口一列。可读且未优化的配置默认纳入检查建议；先检查，确认后执行，原值恢复机制保留。
- 检查结果取消悬浮置顶，九类项目默认折叠，只显示总数与已选数。顶部分类可直接展开相应类别；每类先显示 10 项，继续加载即可看到完整结果。缓存文件明细有独立滚动范围，不让上百条明细撑长整页。个人大文件、应用卸载、需判断的网络操作不因配置默认勾选而被自动选择。
- 清理类型增加简短用途解释；WinGet 相关按钮改为“修复软件安装工具”“刷新软件安装列表”，说明具体处理什么故障。
- 设置里的大面积许可证列表移除，关于保留一个“开源声明”入口。源码与随包许可信息保留。已安装组件的打开、修复、更新、卸载操作统一。

## 社区调研与配置取舍

在已有 11 组配置上新增五组：关闭活动历史上传、输入个性化收集、在线语音识别、Edge 后台驻留，以及减少 Edge 推广。沿用固定规则、首次原值备份、写入读回和失败回退；不把注册表目标值符合等同于所有版本都能彻底停止对应行为。

核对 [WinUtil 配置](https://github.com/ChrisTitusTech/winutil/blob/main/config/tweaks.json)、[Win11Debloat 配置](https://github.com/Raphire/Win11Debloat/blob/master/Regfiles/Disable_Telemetry.reg) 与 Edge [启动预加载](https://learn.microsoft.com/en-us/deployedge/microsoft-edge-policies/StartupBoostEnabled)、[后台运行](https://learn.microsoft.com/en-us/deployedge/microsoft-edge-policies/BackgroundModeEnabled)、[推荐](https://learn.microsoft.com/en-us/deployedge/microsoft-edge-policies/ShowRecommendationsEnabled)、[个性化报告](https://learn.microsoft.com/en-us/deployedge/microsoft-edge-policies/PersonalizationReportingEnabled) 策略。Edge 部分策略受浏览器版本和账号类型限制；不卸载 Edge 或 WebView，不影响依赖它们的应用。

国内参考 [B 站教程介绍](https://www.bilibili.com/video/BV1uAsueFEhP/) 与 [不忘初心系统介绍](https://www.423down.com/17158.html)。后者是转载功能列表，未取得作者原始优化脚本，也未安装其系统；B 站仅核对可读介绍，未声称完整观看视频。列表用于发现候选，实际规则再与开源配置和政策逐项核对。关闭防护/防火墙/更新、删恢复组件、通用停用 SysMain、计时器与虚拟内存改动未作为默认规则加入；没有证据证明这些对所有设备都有性能收益。

## 新增组件

| 组件 | 用途 | 安装与更新来源 |
| --- | --- | --- |
| [Piik](https://github.com/TNTcraftHIM/Piik) | 用户确认的屏幕共享工具，观看方使用浏览器 | 作者 GitHub 正式发布的 Windows amd64 App ZIP |
| [RustDesk](https://github.com/rustdesk/rustdesk) | 远程协助与桌面控制 | 作者 GitHub 正式发布的 x86_64 EXE，作为便携程序管理 |
| [LocalSend](https://github.com/localsend/localsend) | 同一局域网传文件 | WinGet 精确包 LocalSend.LocalSend |

RustDesk 的 WinGet 条目已按发布者要求移除，见 [微软包库记录](https://github.com/microsoft/winget-pkgs/issues/352094)，因此不用不存在的包 ID 安装。Piik 与 RustDesk 的固定 GitHub 源验证 HTTPS 地址、官方 SHA-256 和下载长度；缺少校验信息直接失败。ZIP 验证越界路径、链接、条目数量与解压大小。下载与准备发生在独立目录，校验完成再替换；失败保留旧程序。更新迁移用户配置，卸载只移除记录的程序文件，保留生成的数据和配置。安装不会自动分享屏幕、建立远程连接或设置远程服务。

组件更新检查支持 GitHub 和 WinGet 两条来源，每个组件检查失败独立显示“未核实”。各组件操作保持独立任务；与 WinEase 本身的签名更新是两套流程。

## 验证

- Rust：91 项回归通过，15 项需要显式启动的集成检查默认忽略；新增官方资源约束、正常 ZIP 与越界 ZIP 测试。
- 本机只读：16 组配置均可返回状态；Piik/RustDesk 正式发布资源及 SHA-256 可读取。Piik 官方 ZIP 下载到临时目录，校验通过并确认启动程序，未安装或执行。
- 浏览器模拟 IPC：回归首页确认与选择保留、清理/回收失败、社区管理、系统修复、开关失败及恢复；新增两列、自动滚动、收起结果、分类展开、默认建议、64 块面积比例与非重叠布局、颜色、文件目录入口、组件按钮与开源声明、宽窄布局检查。
- TypeScript/Vite 和原生安装包构建用于本地预览。未执行本机优化或新增组件连接；跨设备共享、远程协助与旧客户端升级仍需实际验收。

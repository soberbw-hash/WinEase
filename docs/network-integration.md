# 网络急救整合

WinEase 将网络急救箱的功能整合到“首页 → 网络修复”，沿用本产品的字体、间距、按钮和确认弹窗。默认路径为：检查网络 → 查看问题 → 修复 → 自动复检。检测详情、专项操作和备份均默认折叠。

## 来源与范围

功能来源：[soberbw-hash/network-first-aid](https://github.com/soberbw-hash/network-first-aid)，审阅基线 `5d468bf85735d6b982c6d8426afc2e5bdf14e6bc`。版权所有者在本次任务中明确要求合并其两个产品。原始许可保留于 `src-tauri/resources/network/SOURCE-LICENSE.txt`，不改变原项目的授权条款。未复制原界面、品牌、赞助页面或独立更新器；本功能随 WinEase 构建。

原仓库和已安装软件不自动删除。旧快照缺少 DNS 自动/手动来源、稳定网卡标识和完整 WinHTTP 状态，不作为本版的自动还原依据，原有数据继续保留在原软件目录。

| 原有能力 | WinEase 实现 |
| --- | --- |
| 网卡、IP、DNS、代理、WinHTTP、TUN、路由、Hosts、进程和服务诊断 | 网卡 GUID、IPv4/IPv6、DNS 模式、手动/PAC 代理、WinHTTP、代理变量是否配置、代理程序图标与路由详情 |
| 直连与代理端口/出口测试 | 两个 HTTPS 站点的直连与系统代理路径、精确内容联网验证、本机端口 TCP 验证、HTTP 代理出口；SOCKS 不冒充 HTTP |
| 智能安全修复 | 后端生成计划；检查结果有效 10 分钟，执行前重新检查，只执行仍存在且已确认的项目 |
| DNS 缓存、DHCP、自动 DNS、失效代理、WinHTTP 清除/同步、局域网例外、残留隧道路由 | 全部集成；常规流程按证据处理，其他操作放在专项修复 |
| 网卡重启、Winsock/TCP-IP、Hosts、防火墙、网络组件重装 | 集成到专项修复，说明连接中断、需重启和无法完整回滚的边界 |
| 自动快照、选择还原、本机审计 | 每次执行前备份；还原范围按动作确定，原来自动的 DNS 不改成静态，使用 GUID 匹配网卡；本机审计日志与脱敏报告 |

## 核心改进

- 不采用人为“网络健康分数”。配置读取失败显示检查不完整，禁用自动修复；端口监听、能获得 HTTP 响应、站点允许访问是不同结果。单个站点失败不触发全量重置。
- 多个代理进程同时存在只是线索，不等于网络冲突；保留能使用的代理/TUN。失效代理必须是所有目标均为已验证的本机端口且全部无法连接，同时没有 PAC 或组织策略。
- 只删除确定断开的非硬件隧道网卡的 ActiveStore 默认/分流路由。不存在的网卡、活动 TUN 和普通物理网卡均不删除。
- DHCP 只续租符合条件的物理网卡，不执行全机 `ipconfig /release`。自动修复只处理 DHCP 物理网卡的 APIPA 地址。自动 DNS 仅作为用户确认的专项操作，保留静态 IP 与虚拟网卡。
- DNS 备份同时记录 DHCP/静态来源和 IPv4/IPv6 手动服务器；还原使用原模式和 GUID。代理仅还原动作涉及的值，保留注册表值类型与原先不存在的状态。
- WinHTTP 通过系统 API 读写静态代理，而非根据中英文命令输出猜测地址；高级自动代理配置不修改。用户代理、WinHTTP 与客户端自己的代理并非通用同步关系。
- 防火墙必须先成功导出非空策略文件才允许重置。Hosts 在备份后变化则拒绝覆盖。网卡关闭失败仍尝试重新启用。检查每个原生命令退出码，保留部分完成的日志。
- 修复/还原前均先保存快照；成功执行命令不代表恢复联网，因此自动复检并显示剩余问题。Winsock、网络栈、DHCP 租约与网卡重装不能依靠配置快照完整回滚，界面明确说明。
- 管理员操作由当前 WinEase EXE 的固定助手入口执行，渲染层只提交动作 ID。助手执行内置脚本，拒绝任意命令、路径穿越、链接和其他管理员账户的 HKCU 操作。实际 PowerShell 命令及子进程置于 Windows Job Object，超时一起终止。管理员任务未返回时阻止重复修复，保留结果与备份。
- 检测和报告不会上传系统配置。导出报告省略用户名/SID、IP、PAC URL、代理字符串、Hosts 内容和进程路径。代理变量仅显示是否配置，不显示可能含凭据的值，也不自动删除。

## 调研依据

带宽测速使用 [Cloudflare 官方测速接口](https://github.com/cloudflare/speedtest)，下载 `__down?bytes=N`、上传 `__up?bytes=N`，不引入会上传汇总结果的第三方 JS SDK。原生 HttpClient 沿用 Windows 系统代理/PAC 与现有 TUN；上传内容由随机数生成，下载禁用解压/缓存并验证长度。四路并发由 64 KiB 逐级增大请求，每个方向最多 8 秒，下载请求上限 96 MiB、上传 48 MiB；失败请求也占用上限，不无限重试。速率为实际接收/服务器确认的字节乘 8 除以测量时间，使用十进制 Mbps。延迟为预热后 3 次 HTTP 请求的中位数，抖动为相邻样本差的平均值；不是 ICMP ping。支持停止及离开页面时取消，停止会终止测量进程和子进程。单项失败保留为空并显示原因，不伪造 0 Mbps。达到流量上限可能使高速线路测量样本较短，跨境服务器、代理和请求开销也会影响结果，不将其标为运营商带宽认证值。测速服务可看到请求来源 IP，界面在操作前显示服务和数据量。

- [Microsoft：修复 Wi-Fi 连接问题](https://support.microsoft.com/en-us/windows/experience/connectivity-networking/fix-wi-fi-connection-issues-in-windows)：网络重置应作为最后手段，可能需要重装 VPN 或虚拟交换机。
- [Microsoft：netsh winhttp](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/netsh-winhttp)：WinHTTP 属于使用此 API 的程序；重置静态代理意味着 DIRECT，不代表所有客户端的网络修复。
- [Microsoft：Set-DnsClientServerAddress](https://learn.microsoft.com/en-us/powershell/module/dnsclient/set-dnsclientserveraddress)：手动配置会覆盖 DHCP 提供的 DNS；ResetServerAddresses 用于恢复默认来源，因此不能把诊断读取到的有效 DNS 一律保存成静态地址。

## 验证与边界

`scripts/test-network-speed.ps1` 的 7 项隔离验证覆盖 Mbps 换算、零时长、下载实际字节、上传确认和流量上限、延迟/抖动、HTTP 失败与压缩响应拒绝。Rust 另验证预先停止、运行中停止及空结果。`-Live` 是显式的小流量接口验证，下载/上传各 128 KiB，确认两个真实端点收发完成；不是完整带宽测量，不作为测速准确度证明。完整测速集成测试默认忽略，以避免自动测试消耗大流量。

Rust 单元测试覆盖代理端点、多协议/IPv6、完整失效判断、正常代理保护、单站失败、配置读取失败、备份路径、限定还原、Unicode 执行与其他管理员账户拒绝。`scripts/test-network-safety.ps1` 对生产脚本运行隔离夹具，验证组织策略、远程会话、静态 IP、PAC、高级 WinHTTP、原生命令失败、网卡重启恢复和 DNS 模式还原；不会写入真实网络配置。

当前机器的只读集成检查读取配置、运行联网测试并比较检测前后的代理、DNS、路由，未执行修复。浏览器界面验证使用相同扫描结果和隔离 IPC 夹具，不等于真实 UAC 修复验证。管理员授权、物理网卡重启、防火墙重置、VPN/静态 IP 环境和重启后的结果，需要隔离 Windows 测试机继续验证，不能以本机诊断通过替代。

后续重点是增加隔离测试机上的断网回归样例、IPv6-only/认证门户/组织网络验证、针对具体应用的可选目标诊断。保持“检查和修复”主流程，不增加测速排行或一键关闭安全功能。

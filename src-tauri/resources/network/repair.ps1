$stored = Get-Content -LiteralPath $snapshotPath -Raw | ConvertFrom-Json
if ($stored.schema -ne 2) { throw '备份版本无效' }
if ($live.collectionErrors.Count -gt 0) { throw '配置重新读取失败，未执行修复' }
if ($actionId -in @('renew-dhcp','restart-active-adapters','reset-winsock','reset-tcpip','full-network-reset') -and $stored.raw.remoteSession) { throw '当前为远程会话，拒绝执行会中断连接的修复' }
if ($live.managed -and $actionId -notin @('flush-dns','remove-orphan-tun-routes')) { throw '网络配置受组织管理，请联系管理员处理专项修复' }
$physical = @($live.adapters | Where-Object { $_.status -eq 'Up' -and $_.hardwareInterface })
function Assert-StaticWinHttp {
  if (!$live.winHttp -or !$live.winHttpDump -or $live.winHttpDump -match '(?i)set\s+advproxy') { throw '当前 WinHTTP 包含高级或自动代理配置，请在系统中管理；未修改配置' }
}
switch ($actionId) {
 'flush-dns' { Flush-Dns }
 'disable-dead-proxy' {
  $proxy=$live.proxy
  if (!$proxy.enabled) { Add-Log '失效代理已关闭，无需再修改';break }
  if ($proxy.autoConfigUrl) { throw '存在 PAC 自动代理，未修改配置' }
  if ($proxy.server -ne $stored.raw.proxy.server) { throw '系统代理已变化，请重新检查' }
  $parts=@($proxy.server -split ';' | Where-Object { $_.Trim() })
  if (!$parts.Count -or $parts.Count -gt 8) { throw '代理格式无法安全解析' }
  foreach($part in $parts) {
   if($part.Trim() -notmatch '^(?:(?:http|https|socks)=)?(?:https?://)?(127\.0\.0\.1|localhost|\[::1\]):([0-9]+)$') { throw '包含远程或无法识别的代理，未修改配置' }
   $hostName=$Matches[1].Trim('[',']');$port=[int]$Matches[2];if($port -lt 1 -or $port -gt 65535){throw '代理端口无效'}
   $tcp=[Net.Sockets.TcpClient]::new();$reachable=$false
   try {$t=$tcp.ConnectAsync($hostName,$port);if($t.Wait(1500)){$reachable=$tcp.Connected}}catch{}finally{$tcp.Dispose()}
   if($reachable){throw '代理端口已恢复，保留当前代理；请重新检查'}
  }
  $key='HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings'
  $latest=Get-ItemProperty $key
  if ([string]$latest.ProxyServer -ne $proxy.server -or [string]$latest.AutoConfigURL) {throw '代理状态已变化，未修改'}
  Set-ItemProperty $key -Name ProxyEnable -Type DWord -Value 0
  Add-Log '已关闭指向不可连接本机端口的手动代理'
  Notify-Proxy
 }
 'remove-orphan-tun-routes' {
  $removed=0
  foreach($route in @($stored.raw.routes)) {
   $adapter=$live.adapters | Where-Object { $_.guid -eq $route.guid } | Select-Object -First 1
   if (!$adapter -or !(Test-Tunnel $adapter) -or $adapter.status -notin @('Disconnected','Not Present')) {continue}
   if ($route.destinationPrefix -notin @('0.0.0.0/0','0.0.0.0/1','128.0.0.0/1','::/0','::/1','8000::/1')) {continue}
   # Refuse to remove a refreshed route or an interface which came online after the scan.
   $latest=Get-NetAdapter -InterfaceIndex $adapter.interfaceIndex -IncludeHidden -ErrorAction SilentlyContinue
   if (!$latest -or [string]$latest.Status -notin @('Disconnected','Not Present')) {continue}
   $entries=@(Get-NetRoute -InterfaceIndex $adapter.interfaceIndex -DestinationPrefix $route.destinationPrefix -PolicyStore ActiveStore -ErrorAction SilentlyContinue | Where-Object { [string]$_.NextHop -eq $route.nextHop -and [int]$_.RouteMetric -eq $route.routeMetric })
   foreach($entry in $entries) { $entry | Remove-NetRoute -Confirm:$false -ErrorAction Stop;$removed++ }
  }
  Add-Log ("已清理 {0} 条断开隧道的残留路由" -f $removed)
 }
 'renew-dhcp' {
  $targets=@($physical | Where-Object { $index=$_.interfaceIndex; $live.ips | Where-Object { $_.interfaceIndex -eq $index -and $_.dhcp -and (!$stored.raw.automaticRepair -or @($_.addresses | Where-Object {$_ -like '169.254.*'}).Count -gt 0) } })
  if(!$targets.Count){throw '没有符合条件的 DHCP 物理网卡；未修改静态 IP'}
  foreach($adapter in $targets) {
   $nic=Get-CimInstance Win32_NetworkAdapterConfiguration | Where-Object { $_.SettingID -eq $adapter.guid -and $_.DHCPEnabled } | Select-Object -First 1
   if(!$nic){throw '网卡已变化，请重新检查'}
   $result=Invoke-CimMethod -InputObject $nic -MethodName RenewDHCPLease
   if([int]$result.ReturnValue -notin @(0,1)){throw ('更新 DHCP 失败，返回值 '+$result.ReturnValue)}
   Add-Log ($adapter.name+' 已重新申请 DHCP 租约')
  }
  Flush-Dns
 }
 'dns-auto' {
  $targets=@($physical | Where-Object {$index=$_.interfaceIndex;$live.ips | Where-Object {$_.interfaceIndex -eq $index -and $_.dhcp}})
  if(!$targets.Count){throw '没有已连接的 DHCP 物理网卡；静态 IP 和虚拟网卡不修改'}
  foreach($adapter in $targets){Set-DnsClientServerAddress -InterfaceIndex $adapter.interfaceIndex -ResetServerAddresses;Add-Log ($adapter.name+' DNS 已恢复自动')}
  Flush-Dns
 }
 'reset-winhttp-proxy' {Assert-StaticWinHttp;Set-WinHttp @{accessType=1;server=$null;bypass=$null};Add-Log 'WinHTTP 静态代理已改为直连'}
 'sync-winhttp-proxy' {
  Assert-StaticWinHttp
  if(!$live.proxy.enabled -or !$live.proxy.server -or $live.proxy.autoConfigUrl -or $live.proxy.server -match '(?i)socks='){throw '只支持已启用的手动 HTTP 代理；PAC/SOCKS 不导入'}
  Set-WinHttp @{accessType=3;server=$live.proxy.server;bypass=$live.proxy.bypass};Add-Log '已同步手动 HTTP 代理到 WinHTTP'
 }
 'normalize-proxy-bypass' {
  $key='HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings'
  $required=@('<local>','localhost','127.0.0.1','[::1]','10.*','192.168.*')+(16..31 | ForEach-Object {'172.'+$_+'.*'})
  $current=[string](Get-ItemProperty $key).ProxyOverride
  $all=@($current -split ';')+$required | ForEach-Object {$_.Trim()} | Where-Object {$_} | Select-Object -Unique
  Set-ItemProperty $key -Name ProxyOverride -Type String -Value ($all -join ';');Add-Log '已补充局域网直连例外';Notify-Proxy
 }
 'restart-active-adapters' {
  if(!$physical.Count){throw '没有可重启的已连接物理网卡'}
  foreach($adapter in $physical) {
   $nic=Get-NetAdapter -InterfaceIndex $adapter.interfaceIndex
   if([string]$nic.InterfaceGuid -ne $adapter.guid){throw '网卡标识已变化'}
   try {$nic | Disable-NetAdapter -Confirm:$false;Start-Sleep -Milliseconds 700}
   finally {Get-NetAdapter -InterfaceIndex $adapter.interfaceIndex | Enable-NetAdapter -Confirm:$false}
   Add-Log ($adapter.name+' 已重启')
  }
 }
 'reset-winsock' {Invoke-Native "$env:SystemRoot\System32\netsh.exe" @('winsock','reset')|Out-Null;Add-Log 'Winsock 已重置，需要重启；第三方网络扩展可能需要重装'}
 'reset-tcpip' {
  if(@($live.ips | Where-Object {!$_.dhcp -and $physical.interfaceIndex -contains $_.interfaceIndex}).Count){throw '存在静态 IP，拒绝重置网络栈'}
  Invoke-Native "$env:SystemRoot\System32\netsh.exe" @('int','ipv4','reset')|Out-Null;Add-Log 'IPv4 已重置'
  Invoke-Native "$env:SystemRoot\System32\netsh.exe" @('int','ipv6','reset')|Out-Null;Add-Log 'IPv6 已重置，需要重启'
 }
 'reset-firewall' {
  $path=Join-Path $snapshotDirectory 'firewall.wfw'
  Invoke-Native "$env:SystemRoot\System32\netsh.exe" @('advfirewall','export',$path)|Out-Null
  if(!(Test-Path -LiteralPath $path) -or (Get-Item -LiteralPath $path).Length -eq 0){throw '防火墙备份不完整，已停止重置'}
  Invoke-Native "$env:SystemRoot\System32\netsh.exe" @('advfirewall','reset')|Out-Null;Add-Log '已备份并重置防火墙策略'
 }
 'reset-hosts' {
  $backup=Join-Path $snapshotDirectory 'hosts';if(!(Test-Path -LiteralPath $backup)){throw 'Hosts 备份不可用'}
  $path=Join-Path $env:SystemRoot 'System32\drivers\etc\hosts'
  if((Get-FileHash -LiteralPath $path).Hash -ne (Get-FileHash -LiteralPath $backup).Hash){throw 'Hosts 在备份后已变化，未覆盖文件'}
  [IO.File]::WriteAllText($path,"# Restored by WinEase`r`n127.0.0.1 localhost`r`n::1 localhost`r`n",[Text.UTF8Encoding]::new($false));Add-Log 'Hosts 已重置';Flush-Dns
 }
 'full-network-reset' {
  if(@($live.ips | Where-Object {!$_.dhcp -and $physical.interfaceIndex -contains $_.interfaceIndex}).Count){throw '存在静态 IP，拒绝重装网络组件'}
  Invoke-Native "$env:SystemRoot\System32\netcfg.exe" @('-d') | Out-Null;Add-Log '网络组件已排队重新安装，需要重启；VPN 和虚拟网卡可能需要重装'
 }
 default {throw '动作未列入修复白名单'}
}

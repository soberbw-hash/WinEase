$stored = Get-Content -LiteralPath $snapshotPath -Raw | ConvertFrom-Json
if($stored.schema -ne 2 -or $stored.userSid -ne $expectedSid){throw '备份格式或用户标识不匹配'}
if($live.managed){throw '当前网络受组织管理，请联系管理员还原配置'}
foreach($scope in @($scopes)){if($scope -notin @('proxy','proxy-enable','proxy-bypass','dns','hosts','firewall','winhttp','routes')){throw '还原范围无效'}}
if($scopes -contains 'winhttp' -and (!$stored.raw.winHttp -or !$stored.raw.winHttpDump -or $stored.raw.winHttpDump -match '(?i)set\s+advproxy' -or $live.winHttpDump -match '(?i)set\s+advproxy')){throw '高级 WinHTTP 配置不支持自动还原；尚未修改其他配置'}
foreach($pair in @(@('hosts','hosts'),@('firewall','firewall.wfw'))){if($scopes -contains $pair[0] -and !(Test-Path -LiteralPath (Join-Path $snapshotDirectory $pair[1]))){throw '备份文件不完整，尚未修改配置'}}
if(@($scopes | Where-Object {$_ -like 'proxy*'}).Count){
 $names=if($scopes -contains 'proxy'){@('ProxyEnable','ProxyServer','ProxyOverride','AutoConfigURL','AutoDetect')}else{@();if($scopes -contains 'proxy-enable'){'ProxyEnable'};if($scopes -contains 'proxy-bypass'){'ProxyOverride'}}
 $key=[Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Software\Microsoft\Windows\CurrentVersion\Internet Settings')
 try {
  foreach($name in $names){
   $value=@($stored.raw.proxyValues | Where-Object name -eq $name)
   if($value.Count -ne 1){throw '代理备份值缺失'};$entry=$value[0]
   if(!$entry.exists){$key.DeleteValue($name,$false);continue}
   if($entry.kind -notin @('DWord','String','ExpandString')){throw '代理备份值类型异常'}
   if($entry.kind -eq 'DWord'){$data=[int]$entry.value}else{$data=[string]$entry.value;if($data.Length -gt 8192){throw '代理备份值过长'}}
   $key.SetValue($name,$data,[Enum]::Parse([Microsoft.Win32.RegistryValueKind],[string]$entry.kind))
  }
 } finally {$key.Dispose()}
 Add-Log '已还原本次操作涉及的用户代理值';Notify-Proxy
}
if($scopes -contains 'dns'){
 $count=0
 $targets=if($stored.actions -contains 'manual'){@($stored.raw.adapters)}else{@($stored.raw.adapters | Where-Object {$index=$_.interfaceIndex; $_.hardwareInterface -and $_.status -eq 'Up' -and @($stored.raw.ips | Where-Object {$_.interfaceIndex -eq $index -and $_.dhcp}).Count -gt 0})}
 foreach($entry in @($stored.raw.dns)){
  if($targets.guid -notcontains $entry.guid){continue}
  $adapter=Get-NetAdapter -IncludeHidden | Where-Object {[string]$_.InterfaceGuid -eq $entry.guid} | Select-Object -First 1
  if(!$adapter){throw '原网卡已不存在，无法完整还原 DNS'}
  $family=if([int]$entry.addressFamily -eq 2){'IPv4'}elseif([int]$entry.addressFamily -eq 23){'IPv6'}else{throw 'DNS 地址族无效'}
  $dnsObject=Get-DnsClientServerAddress -InterfaceIndex $adapter.ifIndex -AddressFamily $family
  if($entry.automatic){$dnsObject|Set-DnsClientServerAddress -ResetServerAddresses}
  else {
   $servers=@($entry.staticServers);if(!$servers.Count -or $servers.Count -gt 16){throw '手动 DNS 备份无效'}
   foreach($server in $servers){$ip=$null;if(![Net.IPAddress]::TryParse([string]$server,[ref]$ip)){throw 'DNS 地址无效'}}
   $dnsObject|Set-DnsClientServerAddress -ServerAddresses $servers
  }
  $count++
 }
 Add-Log ("已按原自动/手动模式还原 {0} 组 DNS" -f $count);Flush-Dns
}
if($scopes -contains 'winhttp'){
 if(!$stored.raw.winHttp -or !$stored.raw.winHttpDump -or $stored.raw.winHttpDump -match '(?i)set\s+advproxy' -or $live.winHttpDump -match '(?i)set\s+advproxy'){throw '高级 WinHTTP 配置不支持自动还原'}
 Set-WinHttp $stored.raw.winHttp;Add-Log 'WinHTTP 静态代理已还原'
}
if($scopes -contains 'routes'){
 $count=0
 foreach($route in @($stored.raw.routes)){
  $original=$stored.raw.adapters|Where-Object {$_.guid -eq $route.guid}|Select-Object -First 1
  if(!$original -or !(Test-Tunnel $original) -or $original.status -notin @('Disconnected','Not Present')){continue}
  if($route.destinationPrefix -notin @('0.0.0.0/0','0.0.0.0/1','128.0.0.0/1','::/0','::/1','8000::/1')){throw '路由范围无效'}
  $adapter=Get-NetAdapter -IncludeHidden | Where-Object {[string]$_.InterfaceGuid -eq $route.guid}|Select-Object -First 1
  if(!$adapter){throw '原隧道网卡已不存在，无法还原路由'}
  $ip=$null;if(![Net.IPAddress]::TryParse([string]$route.nextHop,[ref]$ip)){throw '路由网关无效'}
  $exists=@(Get-NetRoute -PolicyStore ActiveStore -InterfaceIndex $adapter.ifIndex -DestinationPrefix $route.destinationPrefix -ErrorAction SilentlyContinue | Where-Object {[string]$_.NextHop -eq $route.nextHop})
  if(!$exists.Count){New-NetRoute -PolicyStore ActiveStore -InterfaceIndex $adapter.ifIndex -DestinationPrefix $route.destinationPrefix -NextHop $route.nextHop -RouteMetric ([uint16]$route.routeMetric)|Out-Null;$count++}
 }
 Add-Log ("已还原 {0} 条原隧道路由" -f $count)
}
if($scopes -contains 'hosts'){
 $path=Join-Path $snapshotDirectory 'hosts';if(!(Test-Path -LiteralPath $path)){throw 'Hosts 备份不存在'}
 Copy-Item -LiteralPath $path -Destination (Join-Path $env:SystemRoot 'System32\drivers\etc\hosts') -Force;Add-Log 'Hosts 原文件已还原';Flush-Dns
}
if($scopes -contains 'firewall'){
 $path=Join-Path $snapshotDirectory 'firewall.wfw';if(!(Test-Path -LiteralPath $path)){throw '此备份不包含防火墙策略文件'}
 Invoke-Native "$env:SystemRoot\System32\netsh.exe" @('advfirewall','import',$path)|Out-Null;Add-Log '防火墙策略已还原'
}

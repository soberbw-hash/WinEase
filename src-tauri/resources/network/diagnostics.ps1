$errors = [Collections.Generic.List[string]]::new()
function Collect([string]$name, [scriptblock]$body) {
  try { & $body } catch { $errors.Add($name + '：' + $_.Exception.Message) }
}
$internet = Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings' -ErrorAction SilentlyContinue
$adapters = @(Collect '网卡' { Get-NetAdapter -IncludeHidden | ForEach-Object {
  [ordered]@{name=[string]$_.Name;description=[string]$_.InterfaceDescription;status=[string]$_.Status;linkSpeed=[string]$_.LinkSpeed;interfaceIndex=[int]$_.ifIndex;guid=[string]$_.InterfaceGuid;hardwareInterface=[bool]$_.HardwareInterface}
}})
$dns = @(Collect 'DNS' { Get-DnsClientServerAddress | ForEach-Object {
  $entry = $_; $adapter = $adapters | Where-Object interfaceIndex -eq $entry.InterfaceIndex | Select-Object -First 1
  $family = if ([int]$entry.AddressFamily -eq 2) {'Tcpip'} else {'Tcpip6'}
  $key = 'HKLM:\SYSTEM\CurrentControlSet\Services\' + $family + '\Parameters\Interfaces\' + $adapter.guid
  $static = [string](Get-ItemProperty $key -Name NameServer -ErrorAction SilentlyContinue).NameServer
  [ordered]@{interfaceAlias=[string]$entry.InterfaceAlias;interfaceIndex=[int]$entry.InterfaceIndex;guid=$adapter.guid;addressFamily=[int]$entry.AddressFamily;serverAddresses=@($entry.ServerAddresses);staticServers=@($static -split '[,;\s]+' | Where-Object {$_});automatic=[string]::IsNullOrWhiteSpace($static)}
}})
$routes = @(Collect '路由' { Get-NetRoute -PolicyStore ActiveStore | Where-Object DestinationPrefix -in @('0.0.0.0/0','0.0.0.0/1','128.0.0.0/1','::/0','::/1','8000::/1') | ForEach-Object {
  $route = $_; $adapter = $adapters | Where-Object interfaceIndex -eq $route.InterfaceIndex | Select-Object -First 1
  [ordered]@{interfaceAlias=[string]$route.InterfaceAlias;interfaceIndex=[int]$route.InterfaceIndex;guid=$adapter.guid;destinationPrefix=[string]$route.DestinationPrefix;nextHop=[string]$route.NextHop;routeMetric=[int]$route.RouteMetric;interfaceMetric=[int]$route.InterfaceMetric;protocol=[string]$route.Protocol}
}})
$ips = @(Collect 'IP 地址' { Get-NetIPInterface | Where-Object AddressFamily -eq IPv4 | ForEach-Object {
  $entry=$_; $adapter=$adapters | Where-Object interfaceIndex -eq $entry.InterfaceIndex | Select-Object -First 1
  [ordered]@{interfaceIndex=[int]$entry.InterfaceIndex;interfaceAlias=[string]$entry.InterfaceAlias;guid=$adapter.guid;dhcp=([string]$entry.Dhcp -eq 'Enabled');addresses=@(Get-NetIPAddress -InterfaceIndex $entry.InterfaceIndex -ErrorAction SilentlyContinue | ForEach-Object {[string]$_.IPAddress})}
}})
$processes = @(Collect '代理进程' { Get-Process | Where-Object ProcessName -match 'clash|mihomo|xsus|sing-box|v2ray|xray|wireguard|openvpn|tailscale|shadowsocks|trojan' | ForEach-Object {
  $path='';try{$path=[string]$_.Path}catch{}
  [ordered]@{name=[string]$_.ProcessName;id=[int]$_.Id;path=$path}
}})
$services = @(Collect '代理服务' {Get-Service | Where-Object Name -match 'clash|mihomo|xsus|sing-box|v2ray|xray|wireguard|openvpn|tailscale' | ForEach-Object {[ordered]@{name=[string]$_.Name;status=[string]$_.Status}}})
$hostsPath=Join-Path $env:SystemRoot 'System32\drivers\etc\hosts'
$hosts=@(Collect 'Hosts' {Get-Content -LiteralPath $hostsPath | Where-Object { $_.Trim() -and !$_.Trim().StartsWith('#') } | Select-Object -First 100})
$winHttp=Collect 'WinHTTP' {Get-WinHttp}
$winHttpDump=Collect 'WinHTTP 模式' {Invoke-Native "$env:SystemRoot\System32\netsh.exe" @('winhttp','dump')}
$managed=$false
$policy=@('HKCU:\Software\Policies\Microsoft\Windows\CurrentVersion\Internet Settings','HKLM:\Software\Policies\Microsoft\Windows\CurrentVersion\Internet Settings')
foreach($key in $policy){
  $settings=Get-ItemProperty $key -ErrorAction SilentlyContinue
  if($settings -and @($settings.PSObject.Properties.Name | Where-Object {$_ -in @('ProxySettingsPerUser','ProxyEnable','ProxyServer','AutoConfigURL','AutoDetect')}).Count -gt 0){$managed=$true}
}
$computer=Collect '组织策略' {Get-CimInstance Win32_ComputerSystem}
if($computer.PartOfDomain){$managed=$true}
$dnsTests=@()
foreach($hostName in @('www.baidu.com','www.microsoft.com')) {
  $watch=[Diagnostics.Stopwatch]::StartNew();$ok=$false;$detail=''
  try {$task=[Net.Dns]::GetHostAddressesAsync($hostName); if(!$task.Wait(3000)){throw '解析超时'};$ok=($task.Result.Count -gt 0);$detail=($task.Result | ForEach-Object {$_.IPAddressToString}) -join ', '}catch{$detail=$_.Exception.GetBaseException().Message}
  $dnsTests += [ordered]@{name=$hostName;ok=$ok;latencyMs=$watch.ElapsedMilliseconds;detail=$detail}
}
[ordered]@{
 capturedAt=[DateTime]::UtcNow.ToString('o');computerName=$env:COMPUTERNAME;userSid=[Security.Principal.WindowsIdentity]::GetCurrent().User.Value;managed=$managed;remoteSession=(($env:SESSIONNAME -match '^RDP') -or [WinEaseRemote]::GetSystemMetrics(0x1000) -ne 0)
 proxy=[ordered]@{enabled=([int]$internet.ProxyEnable -eq 1);server=[string]$internet.ProxyServer;bypass=[string]$internet.ProxyOverride;autoConfigUrl=[string]$internet.AutoConfigURL}
 proxyValues=@(Get-ProxyValues);winHttp=$winHttp;winHttpDump=$winHttpDump
 adapters=$adapters;dns=$dns;routes=$routes;ips=$ips;proxyProcesses=$processes;proxyServices=$services;hostsEntries=$hosts;dnsTests=$dnsTests;collectionErrors=@($errors)
 environmentProxy=@('HTTP_PROXY','HTTPS_PROXY','ALL_PROXY','NO_PROXY') | ForEach-Object { [ordered]@{name=$_;configured=[bool][Environment]::GetEnvironmentVariable($_,'User');machineConfigured=[bool][Environment]::GetEnvironmentVariable($_,'Machine')} }
} | ConvertTo-Json -Depth 12 -Compress

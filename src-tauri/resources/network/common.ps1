# Network First Aid functionality, integrated and hardened for WinEase.
# Copyright (c) 2026 soberbw-hash. See SOURCE-LICENSE.txt and docs/network-integration.md.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$InformationPreference = 'SilentlyContinue'
$OutputEncoding = [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
if (!( 'WinEaseRemote' -as [type])) {
  Add-Type 'using System.Runtime.InteropServices; public class WinEaseRemote { [DllImport("user32.dll")] public static extern int GetSystemMetrics(int index); }'
}
function Invoke-Native([string]$program, [string[]]$arguments) {
  $output = & $program @arguments 2>&1
  if ($LASTEXITCODE -ne 0) { throw ($program + ' 执行失败：' + ($output -join ' ')) }
  return ($output -join "`n")
}
function Get-ProxyValues {
  $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Internet Settings')
  $values = @()
  try {
    foreach ($name in @('ProxyEnable','ProxyServer','ProxyOverride','AutoConfigURL','AutoDetect')) {
      $exists = $key -and ($key.GetValueNames() -contains $name)
      $values += [ordered]@{name=$name; exists=[bool]$exists; kind=if($exists){[string]$key.GetValueKind($name)}else{''}; value=if($exists){$key.GetValue($name,$null,[Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)}else{$null}}
    }
  } finally { if ($key) {$key.Dispose()} }
  return $values
}
function Test-Tunnel($adapter) {
  return !$adapter.hardwareInterface -and (($adapter.name + ' ' + $adapter.description) -match 'tun|tap|wintun|clash|mihomo|xsus|sing-box|wireguard|openvpn|tailscale')
}
function Flush-Dns {
  Invoke-Native "$env:SystemRoot\System32\ipconfig.exe" @('/flushdns') | Out-Null
  Add-Log 'DNS 缓存已刷新'
}
function Notify-Proxy {
  if (!( 'WinEaseInternet' -as [type])) {
    Add-Type 'using System; using System.Runtime.InteropServices; public class WinEaseInternet { [DllImport("wininet.dll")] public static extern bool InternetSetOption(IntPtr h, int o, IntPtr b, int l); }'
  }
  if (![WinEaseInternet]::InternetSetOption([IntPtr]::Zero,39,[IntPtr]::Zero,0) -or ![WinEaseInternet]::InternetSetOption([IntPtr]::Zero,37,[IntPtr]::Zero,0)) { throw '代理设置已写入，但通知 Windows 刷新失败，请重新打开相关应用' }
}
if (!( 'WinEaseWinHttp' -as [type])) {
  Add-Type @'
using System; using System.Runtime.InteropServices;
public class WinEaseWinHttp {
 [StructLayout(LayoutKind.Sequential)] public struct Proxy { public uint AccessType; public IntPtr Server; public IntPtr Bypass; }
 [DllImport("winhttp.dll",SetLastError=true)] public static extern bool WinHttpGetDefaultProxyConfiguration(out Proxy p);
 [DllImport("winhttp.dll",SetLastError=true)] public static extern bool WinHttpSetDefaultProxyConfiguration(ref Proxy p);
 [DllImport("kernel32.dll")] public static extern IntPtr GlobalFree(IntPtr p);
}
'@
}
function Get-WinHttp {
  $p = New-Object WinEaseWinHttp+Proxy
  if (![WinEaseWinHttp]::WinHttpGetDefaultProxyConfiguration([ref]$p)) { throw '读取 WinHTTP 配置失败' }
  try { return [ordered]@{ accessType=$p.AccessType; server=[Runtime.InteropServices.Marshal]::PtrToStringUni($p.Server); bypass=[Runtime.InteropServices.Marshal]::PtrToStringUni($p.Bypass) } }
  finally { if($p.Server -ne [IntPtr]::Zero){[WinEaseWinHttp]::GlobalFree($p.Server)|Out-Null}; if($p.Bypass -ne [IntPtr]::Zero){[WinEaseWinHttp]::GlobalFree($p.Bypass)|Out-Null} }
}
function Set-WinHttp($config) {
  if ([int]$config.accessType -notin @(1,3)) { throw '此 WinHTTP 模式需要手动处理' }
  $p = New-Object WinEaseWinHttp+Proxy
  $p.AccessType = [uint32]$config.accessType
  try {
    if ($config.server) { $p.Server = [Runtime.InteropServices.Marshal]::StringToHGlobalUni([string]$config.server) }
    if ($config.bypass) { $p.Bypass = [Runtime.InteropServices.Marshal]::StringToHGlobalUni([string]$config.bypass) }
    if (![WinEaseWinHttp]::WinHttpSetDefaultProxyConfiguration([ref]$p)) { throw '写入 WinHTTP 配置失败' }
  } finally { if($p.Server -ne [IntPtr]::Zero){[Runtime.InteropServices.Marshal]::FreeHGlobal($p.Server)}; if($p.Bypass -ne [IntPtr]::Zero){[Runtime.InteropServices.Marshal]::FreeHGlobal($p.Bypass)} }
}

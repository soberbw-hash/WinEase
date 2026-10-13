# Isolated execution tests of the production repair/restore scripts. No live network writes.
$ErrorActionPreference='Stop'
$project=Split-Path $PSScriptRoot -Parent
$resources=Join-Path $project 'src-tauri/resources/network'
foreach($file in Get-ChildItem $resources -Filter '*.ps1') {
 $tokens=$null;$errors=$null
 [Management.Automation.Language.Parser]::ParseInput([IO.File]::ReadAllText($file.FullName,[Text.Encoding]::UTF8),[ref]$tokens,[ref]$errors)|Out-Null
 if($errors.Count){throw ($file.Name+': '+($errors.Message -join '; '))}
}
$repair=[scriptblock]::Create([IO.File]::ReadAllText((Join-Path $resources 'repair.ps1'),[Text.Encoding]::UTF8))
$restoreText=[IO.File]::ReadAllText((Join-Path $resources 'restore.ps1'),[Text.Encoding]::UTF8)
$directory=Join-Path ([IO.Path]::GetTempPath()) ('winease-network-tests-'+[guid]::NewGuid())
$registryFixture='Software\WinEaseNetworkTests\'+[guid]::NewGuid()
New-Item -ItemType Directory $directory|Out-Null
$snapshotDirectory=$directory;$snapshotPath=Join-Path $directory 'snapshot.json';$expectedSid='S-1-5-21-test'
$script:events=[Collections.Generic.List[string]]::new()
function Add-Log([string]$text){$script:events.Add('log:'+ $text)}
function Notify-Proxy{$script:events.Add('notify')}
function Flush-Dns{$script:events.Add('flush')}
function Invoke-Native([string]$program,[string[]]$arguments){$script:events.Add('native:'+($arguments -join ' '));if($script:nativeFailure){throw 'fixture native failure'};return 'fixture'}
function Test-Tunnel($adapter){return !$adapter.hardwareInterface -and $adapter.name -match 'tun'}
function Set-ItemProperty{param($Path,$Name,$Value,$Type);$script:events.Add('proxy:'+ $Name)}
function Get-ItemProperty{param($Path);return $live.proxy}
function Get-NetAdapter{param($InterfaceIndex,[switch]$IncludeHidden);return [pscustomobject]@{InterfaceGuid='guid-1';ifIndex=7;Status='Up'}}
function Set-WinHttp($config){$script:events.Add('winhttp')}
function Set-DnsClientServerAddress{param($InterfaceIndex,$ServerAddresses,[switch]$ResetServerAddresses,[Parameter(ValueFromPipeline)]$InputObject);process{$script:events.Add('dns:'+$(if($ResetServerAddresses){'auto'}else{($ServerAddresses -join ',')}))}}
function Get-DnsClientServerAddress{param($InterfaceIndex,$AddressFamily);[pscustomobject]@{InterfaceIndex=$InterfaceIndex;AddressFamily=$AddressFamily}}
function Get-CimInstance{param($ClassName);[pscustomobject]@{SettingID='guid-1';DHCPEnabled=$true}}
function Invoke-CimMethod{param($InputObject,$MethodName);$script:events.Add('dhcp-renew');[pscustomobject]@{ReturnValue=0}}
function Disable-NetAdapter{param([Parameter(ValueFromPipeline)]$InputObject,[switch]$Confirm);process{$script:events.Add('adapter-disable');throw 'fixture disable failure'}}
function Enable-NetAdapter{param([Parameter(ValueFromPipeline)]$InputObject,[switch]$Confirm);process{$script:events.Add('adapter-enable')}}
function Assert([bool]$condition,[string]$message){if(!$condition){throw $message}}
function Reset-Fixture {
 $script:events.Clear();$script:nativeFailure=$false
 $script:live=[pscustomobject]@{managed=$false;collectionErrors=@();adapters=@([pscustomobject]@{name='Ethernet';description='Ethernet';status='Up';hardwareInterface=$true;interfaceIndex=7;guid='guid-1'});ips=@([pscustomobject]@{interfaceIndex=7;dhcp=$true;addresses=@('192.168.1.9')});proxy=[pscustomobject]@{enabled=$true;server='127.0.0.1:7897';autoConfigUrl='';bypass='localhost'};winHttp=[pscustomobject]@{accessType=1};winHttpDump='reset proxy'}
 $script:fixture=[pscustomobject]@{schema=2;userSid=$expectedSid;actions=@();raw=[pscustomobject]@{remoteSession=$false;automaticRepair=$false;proxy=$live.proxy;adapters=$live.adapters;ips=$live.ips;routes=@();dns=@()}}
 $fixture|ConvertTo-Json -Depth 12|Set-Content -LiteralPath $snapshotPath -Encoding UTF8
}
function Expect-Blocked([string]$id){$script:actionId=$id;$blocked=$false;try{& $repair}catch{$blocked=$true};Assert $blocked ('Expected refusal: '+$id);Assert ($script:events.Count -eq 0) ('Refused operation mutated configuration: '+$id)}
$count=0
try{
 Reset-Fixture;$live.managed=$true;Expect-Blocked 'dns-auto';$count++
 Reset-Fixture;$live.collectionErrors=@('fixture');Expect-Blocked 'reset-winsock';$count++
 Reset-Fixture;$live.ips[0].dhcp=$false;Expect-Blocked 'renew-dhcp';$count++
 Reset-Fixture;$live.ips[0].dhcp=$false;Expect-Blocked 'dns-auto';$count++
 Reset-Fixture;$live.ips[0].dhcp=$false;Expect-Blocked 'reset-tcpip';$count++
 Reset-Fixture;$fixture.raw.remoteSession=$true;$fixture|ConvertTo-Json -Depth 12|Set-Content $snapshotPath -Encoding UTF8;Expect-Blocked 'restart-active-adapters';$count++
 Reset-Fixture;$live.proxy.autoConfigUrl='https://example/pac';Expect-Blocked 'disable-dead-proxy';$count++
 Reset-Fixture;$live.winHttpDump='set advproxy setting-scope=machine';Expect-Blocked 'reset-winhttp-proxy';$count++
 Reset-Fixture;$actionId='reset-firewall';$script:nativeFailure=$true;$blocked=$false;try{& $repair}catch{$blocked=$true};Assert $blocked 'Native failure must surface';Assert (!$script:events.Contains('native:advfirewall reset')) 'Failed export must prevent firewall reset';$count++
 Reset-Fixture;$actionId='restart-active-adapters';try{& $repair}catch{};Assert ($script:events.Contains('adapter-enable')) 'Failed disable must still attempt re-enable';$count++
 Reset-Fixture;$actionId='renew-dhcp';& $repair;Assert ($script:events.Contains('dhcp-renew')) 'Must renew DHCP';Assert (!@($script:events|Where-Object {$_ -match 'release'}).Count) 'Must not release all network addresses';$count++
 Reset-Fixture;$fixture.actions=@('dns-auto');$fixture.raw.dns=@([pscustomobject]@{interfaceIndex=7;guid='guid-1';addressFamily=2;automatic=$true;staticServers=@();serverAddresses=@('192.168.1.1')},[pscustomobject]@{interfaceIndex=7;guid='guid-1';addressFamily=23;automatic=$false;staticServers=@('::1');serverAddresses=@('::1')});$fixture|ConvertTo-Json -Depth 12|Set-Content $snapshotPath -Encoding UTF8;$scopes=@('dns');& ([scriptblock]::Create($restoreText));Assert ($script:events.Contains('dns:auto')) 'DHCP DNS must stay automatic';Assert ($script:events.Contains('dns:::1')) 'Static IPv6 DNS must restore';$count++
 Reset-Fixture;$actionId='run-arbitrary-command';Expect-Blocked $actionId;$count++
 Reset-Fixture;$fixture.raw|Add-Member -NotePropertyName proxyValues -NotePropertyValue @([pscustomobject]@{name='ProxyEnable';exists=$true;kind='DWord';value=1},[pscustomobject]@{name='ProxyServer';exists=$true;kind='String';value='127.0.0.1:7897'},[pscustomobject]@{name='ProxyOverride';exists=$true;kind='String';value='localhost'},[pscustomobject]@{name='AutoConfigURL';exists=$false;kind='';value=$null},[pscustomobject]@{name='AutoDetect';exists=$true;kind='DWord';value=0});$fixture|ConvertTo-Json -Depth 12|Set-Content $snapshotPath -Encoding UTF8
 $scopes=@('proxy');$isolated=$restoreText.Replace("Software\Microsoft\Windows\CurrentVersion\Internet Settings",$registryFixture);& ([scriptblock]::Create($isolated))
 $key=[Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($registryFixture);try{Assert ($key.GetValue('ProxyEnable') -eq 1) 'Proxy DWORD restore';Assert ($key.GetValueKind('ProxyEnable') -eq [Microsoft.Win32.RegistryValueKind]::DWord) 'Proxy DWORD kind';Assert ($key.GetValue('ProxyServer') -eq '127.0.0.1:7897') 'Proxy string restore';Assert ($key.GetValueNames() -notcontains 'AutoConfigURL') 'Originally absent PAC remains absent'}finally{$key.Dispose()};$count++
 Write-Output "$count network script safety cases passed; no live network changes."
}finally{
 [Microsoft.Win32.Registry]::CurrentUser.DeleteSubKeyTree($registryFixture,$false)
 $resolved=[IO.Path]::GetFullPath($directory);$tempRoot=[IO.Path]::GetFullPath([IO.Path]::GetTempPath())
 if($resolved.StartsWith($tempRoot,[StringComparison]::OrdinalIgnoreCase)-and (Split-Path $resolved -Leaf).StartsWith('winease-network-tests-')){Remove-Item -LiteralPath $resolved -Recurse -Force}
}

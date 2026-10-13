$ErrorActionPreference='Stop'
function Registry-Base($key) {
 if($key.StartsWith('HKCU\')){return [Microsoft.Win32.Registry]::CurrentUser}
 if($key.StartsWith('HKLM\')){return [Microsoft.Win32.Registry]::LocalMachine}
 throw '不允许的配置位置'
}
function Read-Field($f) {
 $key=(Registry-Base $f.key).OpenSubKey($f.key.Substring(5))
 try {
  $exists=$key -and ($key.GetValueNames() -contains $f.name)
  if($exists -and $key.GetValueKind($f.name) -ne [Microsoft.Win32.RegistryValueKind]::DWord){throw '配置类型不受支持'}
  [pscustomobject]@{key=$f.key;name=$f.name;exists=[bool]$exists;value=if($exists){[uint32]$key.GetValue($f.name)}else{$null}}
 } finally {if($key){$key.Dispose()}}
}
function Write-Field($f,$v) {
 $base=Registry-Base $f.key;$path=$f.key.Substring(5)
 if(!$v.exists){$key=$base.OpenSubKey($path,$true)}else{$key=$base.CreateSubKey($path)}
 try {if($key){if($v.exists){$key.SetValue($f.name,[int]$v.value,[Microsoft.Win32.RegistryValueKind]::DWord)}else{$key.DeleteValue($f.name,$false)}}}finally{if($key){$key.Dispose()}}
 $again=Read-Field $f
 if($again.exists -ne $v.exists -or ($v.exists -and $again.value -ne $v.value)){throw '写入后验证失败'}
}
function Power-Current {
 $text=& "$env:windir\System32\powercfg.exe" /getactivescheme 2>&1
 if($LASTEXITCODE -ne 0){throw ($text -join ' ')}
 $m=[regex]::Match(($text -join ' '),'[a-fA-F0-9]{8}(-[a-fA-F0-9]{4}){3}-[a-fA-F0-9]{12}')
 if(!$m.Success){throw '电源计划无法识别'};$m.Value.ToLowerInvariant()
}
function Power-Target {
 $text=& "$env:windir\System32\powercfg.exe" /list 2>&1
 if($LASTEXITCODE -ne 0){throw ($text -join ' ')}
 foreach($guid in @('e9a42b02-d5df-448d-aa00-03f14749eb61','8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c')){if(($text -join ' ').ToLowerInvariant().Contains($guid)){return $guid}}
 throw '此设备没有可用的高性能计划，不修改现代待机配置'
}
function Snapshot($p) {
 switch($p.kind){
  'registry' {return [pscustomobject]@{entries=@($p.fields|ForEach-Object {Read-Field $_})}}
  'disk' {$t=Get-ScheduledTask -TaskPath '\Microsoft\Windows\Defrag\' -TaskName 'ScheduledDefrag';return [pscustomobject]@{taskEnabled=[bool]$t.Settings.Enabled}}
  'power' {return [pscustomobject]@{power=(Power-Current)}}
  default {throw '未知优化类型'}
 }
}
function Validate-Snapshot($p,$s) {
 switch($p.kind){
  'registry' {
   if(@($s.entries).Count -ne @($p.fields).Count){throw '恢复记录数量不匹配'}
   for($i=0;$i -lt @($p.fields).Count;$i++){
    $r=$s.entries[$i];$f=$p.fields[$i]
    if($r.key -cne $f.key -or $r.name -cne $f.name -or $r.exists -isnot [bool]){throw '恢复记录位置不匹配'}
    if($r.exists -and ($null -eq $r.value -or [long]$r.value -lt 0 -or [long]$r.value -gt [int]::MaxValue)){throw '恢复记录数值异常'}
   }
  }
  'disk' {if($s.taskEnabled -isnot [bool]){throw '磁盘维护恢复记录异常'}}
  'power' {if([string]$s.power -notmatch '^[a-fA-F0-9]{8}(-[a-fA-F0-9]{4}){3}-[a-fA-F0-9]{12}$'){throw '电源恢复记录异常'}}
 }
}
function Apply-Snapshot($p,$s) {
 Validate-Snapshot $p $s
 switch($p.kind){
  'registry' {for($i=0;$i -lt @($p.fields).Count;$i++){Write-Field $p.fields[$i] $s.entries[$i]}}
  'disk' {
   if($s.taskEnabled){Enable-ScheduledTask -TaskPath '\Microsoft\Windows\Defrag\' -TaskName 'ScheduledDefrag'|Out-Null}else{Disable-ScheduledTask -TaskPath '\Microsoft\Windows\Defrag\' -TaskName 'ScheduledDefrag'|Out-Null}
   if((Snapshot $p).taskEnabled -ne $s.taskEnabled){throw '磁盘维护状态校验失败'}
  }
  'power' {& "$env:windir\System32\powercfg.exe" /setactive $s.power|Out-Null;if($LASTEXITCODE -ne 0 -or (Power-Current) -ne $s.power){throw '电源计划切换或校验失败'}}
 }
}
function Is-Optimized($p,$s) {
 switch($p.kind){
  'registry' {for($i=0;$i -lt @($p.fields).Count;$i++){if(!$s.entries[$i].exists -or $s.entries[$i].value -ne $p.fields[$i].on){return $false}};return $true}
  'disk' {return !$s.taskEnabled}
  'power' {return $s.power -eq (Power-Target)}
 }
}
function Backup-Path($id){Join-Path $jobRoot "preset-$id.json"}
function Read-Backup($p) {
 $path=Backup-Path $p.id
 if((Get-Item -LiteralPath $path).Attributes -band [IO.FileAttributes]::ReparsePoint){throw '恢复记录包含链接'}
 $b=Get-Content -LiteralPath $path -Raw|ConvertFrom-Json
 if($b.id -cne $p.id){throw '恢复记录项目不匹配'}
 Validate-Snapshot $p $b.original;return $b.original
}
# Diagnostic level 0 is supported only by selected editions. Preserve required data otherwise.
$edition=(Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion' -Name EditionID).EditionID
foreach($p in $catalog){if($p.id -eq 'diagnostics' -and $edition -match 'Enterprise|Education|IoT'){$p.fields[0].on=0}}
$managed=$false
try {$managed=[bool](Get-CimInstance Win32_ComputerSystem -OperationTimeoutSec 8).PartOfDomain
 # Windows itself maintains many non-MDM enrollments. EnrollmentState alone is not management evidence.
 if(Test-Path 'HKLM:\SOFTWARE\Microsoft\Enrollments'){ $managed=$managed -or [bool](Get-ChildItem 'HKLM:\SOFTWARE\Microsoft\Enrollments' | Where-Object { $e=Get-ItemProperty -LiteralPath $_.PSPath; $e.EnrollmentState -eq 1 -and $e.DiscoveryServiceFullURL -and $e.EnrollmentType -eq 6 }) }
 if(Test-Path 'HKLM:\SOFTWARE\Microsoft\Provisioning\OMADM\Accounts'){ $managed=$managed -or [bool]@(Get-ChildItem 'HKLM:\SOFTWARE\Microsoft\Provisioning\OMADM\Accounts').Count }
} catch { $managed=$true }
if($mode -eq 'read') {
 $rows=@(foreach($p in $catalog){
  $enabled=$null;$errorText=$null;$restore=$false
  try {
   if($p.admin -and $managed){throw '设备由组织管理，不覆盖管理策略'}
   if($p.id -eq 'storage-cleanup' -and $edition -match '^Core') {throw '此版本不支持存储感知组策略'}
   $s=Snapshot $p;$enabled=Is-Optimized $p $s
   if(Test-Path -LiteralPath (Backup-Path $p.id)){ $null=Read-Backup $p;$restore=$true }
  }catch{$errorText=$_.Exception.Message}
  [pscustomobject]@{id=$p.id;label=$p.label;description=$p.description;enabled=$enabled;canRestore=$restore;error=$errorText}
 })
 ConvertTo-Json -InputObject $rows -Depth 6 -Compress
} else {
 $before=$null;$p=$null
 try {
  $p=@($catalog|Where-Object id -CEQ $presetId)|Select-Object -First 1
  if(!$p -or $mode -notin @('on','off')){throw '未知优化操作'}
  if($p.admin -and $managed){throw '设备由组织管理，不覆盖管理策略'}
  if($p.id -eq 'storage-cleanup' -and $edition -match '^Core') {throw '此版本不支持存储感知组策略'}
  $before=Snapshot $p
  $path=Backup-Path $p.id
  if(Test-Path -LiteralPath $path){$original=Read-Backup $p}
  else {
   $original=$before
   # If another tool already optimized this feature, there is no earlier WinEase baseline.
   # Off must still work: use the explicit unoptimized/default scheme and retain observed state for audit.
   if($mode -eq 'off'){
    if($p.kind -eq 'registry'){$original=[pscustomobject]@{entries=@($p.fields|ForEach-Object {[pscustomobject]@{key=$_.key;name=$_.name;exists=$true;value=$_.off}})}}
    elseif($p.kind -eq 'disk'){$original=[pscustomobject]@{taskEnabled=$true}}
    else{$original=[pscustomobject]@{power='381b4222-f694-41f0-9685-ff5bb260df2e'}}
   }
   $bytes=[Text.Encoding]::UTF8.GetBytes((@{id=$p.id;original=$original;firstObserved=$before}|ConvertTo-Json -Depth 8 -Compress))
   $stream=[IO.File]::Open($path,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
   try{$stream.Write($bytes,0,$bytes.Length);$stream.Flush($true)}finally{$stream.Dispose()}
  }
  if($mode -eq 'off'){ $target=$original }
  elseif($p.kind -eq 'registry'){$target=[pscustomobject]@{entries=@($p.fields|ForEach-Object {[pscustomobject]@{key=$_.key;name=$_.name;exists=$true;value=$_.on}})}}
  elseif($p.kind -eq 'disk'){$target=[pscustomobject]@{taskEnabled=$false}}
  else{$target=[pscustomobject]@{power=(Power-Target)}}
  Apply-Snapshot $p $target
  @{success=$true;message=if($mode -eq 'on'){'优化已保存并校验；部分功能在下次启动应用或登录后使用新配置。'}else{'已恢复优化前的配置。'};logs=$p.id;restart=$false}|ConvertTo-Json -Compress
 } catch {
  $message=$_.Exception.Message
  if($before -and $p){try{Apply-Snapshot $p $before;$message+='；本次修改已回退。'}catch{$message+='；回退未完整完成，原配置记录已保留。'}}
  @{success=$false;message=$message;logs=$presetId;restart=$false}|ConvertTo-Json -Compress
 }
}

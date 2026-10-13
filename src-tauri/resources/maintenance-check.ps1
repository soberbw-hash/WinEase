$ErrorActionPreference='Stop'
$rows=[Collections.Generic.List[object]]::new()
function Add-Check($id,$title,$status,$detail,$repair) {$rows.Add([pscustomobject]@{id=$id;title=$title;status=$status;detail=$detail;repair=$repair})}
$pending=(Test-Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\RebootPending') -or (Test-Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired')
if($pending){Add-Check 'update-reboot' '系统更新待重启' 'attention' '先保存工作并重启，再判断是否仍有更新故障。' $null}
foreach($name in @('wuauserv','BITS')) {
    try {
        $service=Get-CimInstance Win32_Service -Filter "Name='$name'" -OperationTimeoutSec 8
        if(!$service){throw '服务不存在'}
        if($service.StartMode -eq 'Disabled'){Add-Check "service-$name" "更新服务 $name" 'attention' '服务被禁用；修复将备份启动模式并改为手动，启动服务。' 'update-services'}
        else {Add-Check "service-$name" "更新服务 $name" 'healthy' "启动模式 $($service.StartMode)；$($service.State)。按需服务停止不等于故障。" $null}
    } catch {Add-Check "service-$name" "更新服务 $name" 'unknown' $_.Exception.Message $null}
}
try {
    $history=(New-Object -ComObject Microsoft.Update.Session).CreateUpdateSearcher()
    $count=$history.GetTotalHistoryCount();$entries=if($count){@($history.QueryHistory(0,[math]::Min($count,20)))}else{@()}
    $latest=@($entries | Where-Object {$_.Operation -eq 1 -and $_.Date -gt (Get-Date).AddDays(-14)} | Select-Object -First 1)
    if($latest.Count -and $latest[0].ResultCode -in @(3,4,5)) {
        Add-Check 'update-history' '最近更新未成功' 'attention' ("最近一次："+$latest[0].Title+"；错误码 "+('0x{0:X8}' -f ($latest[0].HResult -band 0xffffffffL))+"。请查看更新页面，服务修复不能保证解决此错误。") $null
    } else {Add-Check 'update-history' '更新记录' 'healthy' '近期最近一次安装记录未报告失败，或暂无近期安装记录；没有触发联网更新扫描。' $null}
} catch {Add-Check 'update-history' '更新记录' 'unknown' $_.Exception.Message $null}
foreach($entry in @(@('winget','应用安装程序','Microsoft.DesktopAppInstaller'),@('store','Microsoft Store','Microsoft.WindowsStore'))) {
    try {
        $package=Get-AppxPackage -Name $entry[2] | Select-Object -First 1
        if(!$package){Add-Check $entry[0] $entry[1] 'attention' '当前用户未注册此应用；可在商店重新安装。' $null}
        elseif([string]$package.Status -ne 'Ok'){Add-Check $entry[0] $entry[1] 'attention' ("包状态："+$package.Status+'；尝试重新注册，保留应用数据。') ($entry[0]+'-register')}
        else {Add-Check $entry[0] $entry[1] 'healthy' ("已注册，版本 "+$package.Version+'；包注册正常不代表服务器或下载链路正常。') $null}
    } catch {Add-Check $entry[0] $entry[1] 'unknown' $_.Exception.Message $null}
}
try {
    $cmd=Get-Command winget.exe -ErrorAction Stop
    # Version is offline; source update is a separate explicitly requested operation.
    $version=& $cmd.Source --version 2>&1;$code=$LASTEXITCODE
    if($code -ne 0){throw ("WinGet 退出码 "+$code)}
    Add-Check 'winget-command' 'WinGet 命令' 'healthy' ($version -join ' ') $null
} catch {Add-Check 'winget-command' 'WinGet 命令' 'attention' $_.Exception.Message 'winget-register'}
[pscustomobject]@{checks=@($rows.ToArray());pendingReboot=$pending}|ConvertTo-Json -Depth 5 -Compress

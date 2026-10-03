$ErrorActionPreference='Stop'
$healthErrors=[Collections.Generic.List[object]]::new()
function Read-Health([string]$name,[scriptblock]$read) {
    try { & $read } catch { $healthErrors.Add([pscustomobject]@{name=$name;detail=$_.Exception.Message}); return $null }
}
$os=Read-Health 'memory' { Get-CimInstance Win32_OperatingSystem -OperationTimeoutSec 8 }
$drives=Read-Health 'drives' { @(Get-CimInstance Win32_LogicalDisk -Filter 'DriveType=3' -OperationTimeoutSec 8 | ForEach-Object { [pscustomobject]@{name=$_.DeviceID;freeBytes=[uint64]$_.FreeSpace;totalBytes=[uint64]$_.Size} }) }
$defender=Read-Health 'security' { $status=Get-MpComputerStatus; [pscustomobject]@{enabled=[bool]$status.AntivirusEnabled;realtime=[bool]$status.RealTimeProtectionEnabled;signatureAge=$status.AntivirusSignatureAge;mode=[string]$status.AMRunningMode} }
$providers=Read-Health 'providers' { @(Get-CimInstance -Namespace root/SecurityCenter2 -ClassName AntivirusProduct -OperationTimeoutSec 8 | ForEach-Object { [pscustomobject]@{name=$_.displayName;state=[uint32]$_.productState} }) }
$firewall=Read-Health 'firewall' {
    $profiles=@(Get-NetConnectionProfile)
    $names=@($profiles | ForEach-Object { if($_.NetworkCategory -eq 'DomainAuthenticated'){'Domain'}else{[string]$_.NetworkCategory} } | Select-Object -Unique)
    if(!$names.Count){ return @() }
    @(Get-NetFirewallProfile | Where-Object { $names -contains [string]$_.Name } | ForEach-Object { [pscustomobject]@{name=[string]$_.Name;enabled=[bool]$_.Enabled} })
}
$devices=Read-Health 'devices' { @(Get-CimInstance Win32_PnPEntity -Filter 'ConfigManagerErrorCode <> 0' -OperationTimeoutSec 8 | Where-Object { $_.ConfigManagerErrorCode -ne 22 } | Select-Object -First 30 | ForEach-Object { [pscustomobject]@{name=$_.Name;code=$_.ConfigManagerErrorCode} }) }
$events=Read-Health 'events' {
    try {
        $query="*[System[TimeCreated[timediff(@SystemTime) <= 604800000] and ((Provider[@Name='Microsoft-Windows-Kernel-Power'] and EventID=41) or (Provider[@Name='Microsoft-Windows-WER-SystemErrorReporting'] and EventID=1001) or (Provider[@Name='disk'] and (EventID=7 or EventID=51 or EventID=153)) or ((Provider[@Name='Ntfs'] or Provider[@Name='Microsoft-Windows-Ntfs']) and EventID=55))]]"
        @(Get-WinEvent -LogName System -FilterXPath $query -MaxEvents 100 -ErrorAction Stop | ForEach-Object { [pscustomobject]@{id=$_.Id;provider=$_.ProviderName;time=$_.TimeCreated.ToUniversalTime().ToString('o')} })
    } catch { if($_.FullyQualifiedErrorId -like 'NoMatchingEventsFound*'){return @()}; throw }
}
$reboot=Read-Health 'reboot' { (Test-Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\RebootPending') -or (Test-Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired') }
$pause=Read-Health 'updates' {
    $path='HKLM:\SOFTWARE\Microsoft\WindowsUpdate\UX\Settings'
    if(!(Test-Path $path)){ return $false }
    $settings=Get-ItemProperty -LiteralPath $path
    if(!$settings.PauseUpdatesExpiryTime){ return $false }
    ([DateTime]::Parse([string]$settings.PauseUpdatesExpiryTime).ToUniversalTime() -gt [DateTime]::UtcNow)
}
[pscustomobject]@{
    memoryPercent=if($os -and $os.TotalVisibleMemorySize){[math]::Round((1-$os.FreePhysicalMemory/$os.TotalVisibleMemorySize)*100)}else{$null}
    drives=@($drives | Where-Object { $null -ne $_ });defender=$defender;providers=@($providers | Where-Object { $null -ne $_ });firewall=@($firewall | Where-Object { $null -ne $_ });devices=@($devices | Where-Object { $null -ne $_ });events=@($events | Where-Object { $null -ne $_ })
    pendingReboot=$reboot;updatesPaused=$pause;errors=@($healthErrors.ToArray())
} | ConvertTo-Json -Depth 7 -Compress

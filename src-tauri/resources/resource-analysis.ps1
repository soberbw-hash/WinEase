$ErrorActionPreference='Stop'
$session=(Get-Process -Id $PID).SessionId
$logical=[Environment]::ProcessorCount
$previous=@{};$totals=@{};$watch=[Diagnostics.Stopwatch]::StartNew();$last=0.0
for($sample=0;$sample -le 6;$sample++) {
    if($sample -gt 0){Start-Sleep -Seconds 1}
    $now=$watch.Elapsed.TotalSeconds;$interval=$now-$last;$last=$now
    foreach($p in @(Get-Process | Where-Object {$_.SessionId -eq $session})) {
        try {
            $path=$p.Path;if(!$path){continue}
            $key=[string]$p.Id+':'+[string]$p.StartTime.ToUniversalTime().Ticks
            $cpu=[double]$p.TotalProcessorTime.TotalSeconds
            if($sample -gt 0 -and $previous.ContainsKey($key)) {
                $percent=[math]::Min(100,[math]::Max(0,($cpu-$previous[$key])/$interval/$logical*100))
                if(!$totals.ContainsKey($key)){$totals[$key]=@{pid=$p.Id;stamp=[string]$p.StartTime.ToUniversalTime().Ticks;name=$p.ProcessName;path=$path;cpu=0.0;memory=0.0;count=0}}
                $t=$totals[$key];$t.cpu+=$percent;$t.memory+=$p.WorkingSet64;$t.count++
            }
            $previous[$key]=$cpu
        } catch {}
    }
}
$watch.Stop()
$rows=@($totals.Values | ForEach-Object {[pscustomobject]@{pid=$_.pid;stamp=$_.stamp;name=$_.name;path=$_.path;cpuPercent=[math]::Round($_.cpu/$_.count,1);memoryBytes=[uint64]($_.memory/$_.count);samples=$_.count}})
$boot=$null;$bootError=$null;$startup=@()
try {
    $events=@(Get-WinEvent -FilterHashtable @{LogName='Microsoft-Windows-Diagnostics-Performance/Operational';Id=100;StartTime=(Get-Date).AddDays(-30)} -MaxEvents 1 -ErrorAction Stop)
    if($events.Count) {
        $event=$events[0];$data=@{};([xml]$event.ToXml()).Event.EventData.Data | ForEach-Object {$data[$_.Name]=[string]$_.InnerText}
        if($data.BootTime){$boot=[pscustomobject]@{durationMs=[long]$data.BootTime;time=$event.TimeCreated.ToUniversalTime().ToString('o')}}
        try {
            $startup=@(Get-WinEvent -FilterHashtable @{LogName='Microsoft-Windows-Diagnostics-Performance/Operational';Id=101;StartTime=$event.TimeCreated.AddMinutes(-1);EndTime=$event.TimeCreated.AddMinutes(15)} -MaxEvents 30 -ErrorAction Stop | ForEach-Object {
                $d=@{};([xml]$_.ToXml()).Event.EventData.Data | ForEach-Object {$d[$_.Name]=[string]$_.InnerText}
                [pscustomobject]@{name=$d.Name;path=$d.Path;durationMs=if($d.TotalTime){[long]$d.TotalTime}else{$null};delayMs=if($d.DegradationTime){[long]$d.DegradationTime}else{$null}}
            })
        } catch {if($_.FullyQualifiedErrorId -notlike 'NoMatchingEventsFound*'){$bootError=$_.Exception.Message}}
    }
} catch {$bootError=$_.Exception.Message}
[pscustomobject]@{seconds=[math]::Round($watch.Elapsed.TotalSeconds,1);processes=$rows;boot=$boot;startup=$startup;bootError=$bootError}|ConvertTo-Json -Depth 6 -Compress

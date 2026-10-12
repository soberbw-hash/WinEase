use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    sync::Mutex,
    time::{Duration, Instant},
};
static LOCK: Mutex<()> = Mutex::new(());
#[derive(Serialize, Deserialize)]
pub struct RepairResult {
    pub success: bool,
    pub message: String,
    pub logs: String,
    pub restart: bool,
}
fn directory() -> Result<std::path::PathBuf, String> {
    let root = crate::local_app_data_dir()
        .ok_or("无法定位修复记录")?
        .join("WinToolbox/SystemRepair");
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    if !crate::cleaning::no_reparse_ancestors(&root) {
        return Err("修复目录包含链接".into());
    }
    Ok(root)
}
// These tools repair Windows component/system-file integrity, not disposable file caches.
const REPAIR: &str = r#"
$ErrorActionPreference='Stop'
$logs=[Collections.Generic.List[string]]::new();$restart=$false
try {
 $dism=& "$env:SystemRoot\System32\dism.exe" /Online /Cleanup-Image /RestoreHealth 2>&1
 $code=$LASTEXITCODE;$logs.Add(($dism -join "`n"))
 if($code -notin @(0,3010)){throw ("Windows 组件修复失败，退出码 "+$code)}
 $restart=$code -eq 3010
 $sfc=& "$env:SystemRoot\System32\sfc.exe" /scannow 2>&1
 $code=$LASTEXITCODE;$logs.Add(($sfc -join "`n"))
 if($code -ne 0){throw ("系统文件检查未完成，退出码 "+$code)}
 @{success=$true;message='系统组件修复与文件检查已完成，请查看结果记录。';logs=($logs -join "`n");restart=$restart}|ConvertTo-Json -Compress
} catch { @{success=$false;message=$_.Exception.Message;logs=($logs -join "`n");restart=$restart}|ConvertTo-Json -Compress }
"#;
const UPDATE_SERVICES: &str = r#"
$ErrorActionPreference='Stop'
try {
 if(Test-Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate'){throw '检测到更新管理策略，请由系统管理员确认，不覆盖组织配置'}
 $backup=Join-Path $jobRoot 'update-service-backup.json'
 $names=@('wuauserv','BITS');$rows=@()
 if($operation -eq 'restore-update-services') {
  if(!(Test-Path -LiteralPath $backup)){throw '没有更新服务恢复记录'}
  $rows=@(Get-Content -LiteralPath $backup -Raw | ConvertFrom-Json)
 } else {
  foreach($name in $names) {
   $service=Get-CimInstance Win32_Service -Filter "Name='$name'" -OperationTimeoutSec 8
   if(!$service){throw "找不到服务 $name"}
   if($service.StartMode -eq 'Disabled'){$rows+=[pscustomobject]@{name=$name;mode='Disabled';state=[string]$service.State}}
  }
  if(!$rows.Count){throw '服务已不是禁用状态，请重新检查，无需修改'}
  if(Test-Path -LiteralPath $backup){throw '已有更新服务恢复记录；请先恢复或保留现状，不覆盖原配置'}
  $stream=[IO.File]::Open($backup,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
  try {$bytes=[Text.Encoding]::UTF8.GetBytes((ConvertTo-Json -InputObject $rows -Compress));$stream.Write($bytes,0,$bytes.Length);$stream.Flush($true)}finally{$stream.Dispose()}
 }
 foreach($row in $rows) {
  if($row.name -notin $names -or $row.mode -ne 'Disabled' -or $row.state -notin @('Stopped','Running')){throw '更新服务恢复记录无效'}
 }
 foreach($row in $rows) {
  if($operation -eq 'restore-update-services') {
   if($row.state -eq 'Stopped'){Stop-Service -Name $row.name -ErrorAction Stop}
   $text=& "$env:windir\System32\sc.exe" config $row.name start= disabled 2>&1
   if($LASTEXITCODE -ne 0){throw ($text -join "`n")}
   $again=Get-CimInstance Win32_Service -Filter "Name='$($row.name)'" -OperationTimeoutSec 8
   if($again.StartMode -ne 'Disabled'){throw '服务恢复校验失败'}
  } else {
   $text=& "$env:windir\System32\sc.exe" config $row.name start= demand 2>&1
   if($LASTEXITCODE -ne 0){throw ($text -join "`n")}
   Start-Service -Name $row.name -ErrorAction Stop
   $again=Get-CimInstance Win32_Service -Filter "Name='$($row.name)'" -OperationTimeoutSec 8
   if($again.StartMode -eq 'Disabled' -or $again.State -ne 'Running'){throw '服务修复后状态仍异常'}
  }
 }
 if($operation -eq 'restore-update-services'){Remove-Item -LiteralPath $backup -ErrorAction Stop}
 @{success=$true;message=if($operation -eq 'restore-update-services'){'已恢复更新服务原配置。'}else{'已将被禁用的更新服务改为手动并启动，状态复检通过；原配置可恢复。更新安装结果请在 Windows 更新中复检。'};logs=($rows.name -join ', ');restart=$false}|ConvertTo-Json -Compress
} catch {@{success=$false;message=$_.Exception.Message;logs='已保留原配置记录，可恢复；未删除更新下载或数据库。';restart=$false}|ConvertTo-Json -Compress}
"#;
pub async fn repair_update_services() -> Result<RepairResult, String> {
    run_repair("update-services".into()).await
}
#[tauri::command]
pub async fn restore_update_services(confirmed: bool) -> Result<RepairResult, String> {
    if !confirmed {
        return Err("请先确认恢复更新服务原配置".into());
    }
    run_repair("restore-update-services".into()).await
}
#[tauri::command]
pub async fn repair_windows() -> Result<RepairResult, String> {
    run_repair("system".into()).await
}
pub(crate) async fn run_repair(operation: String) -> Result<RepairResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _lock=LOCK.lock().map_err(|_|"系统维护状态异常")?;
        let root=directory()?;
        // A timed-out elevated helper must not be started again while it is still running.
        for entry in fs::read_dir(&root).map_err(|e|e.to_string())?.flatten() {
            if entry.path().join("pending").exists() && !entry.path().join("result.json").exists() {
                if let Some(pid)=fs::read_to_string(entry.path().join("pid")).ok().and_then(|s|s.trim().parse::<u32>().ok()) {
                    let alive=crate::run_powershell_json(&format!("if(Get-Process -Id {pid} -ErrorAction SilentlyContinue){{'yes'}}else{{'no'}}"))?;
                    if alive.trim()=="no" {let _=fs::remove_file(entry.path().join("pending"));continue;}
                }
                return Err("上一次系统修复尚未结束，请等待结果后再操作".into());
            }
        }
        let id=uuid::Uuid::new_v4().to_string();let job=root.join(&id);fs::create_dir(&job).map_err(|e|e.to_string())?;
        let executable=std::env::current_exe().map_err(|e|e.to_string())?;
        let owner = crate::run_powershell_json("[Security.Principal.WindowsIdentity]::GetCurrent().User.Value")?;
        let owner=owner.trim();
        if !owner.starts_with("S-1-5-") || !owner.chars().all(|c|c.is_ascii_digit()||c=='S'||c=='-') {return Err("当前用户标识无效".into());}
        let root64=STANDARD.encode(root.to_string_lossy().as_bytes());
        fs::write(job.join("pending"),b"pending").map_err(|e|e.to_string())?;
        let launch=format!("$ErrorActionPreference='Stop';Start-Process -FilePath '{}' -ArgumentList '--winease-system-repair {id} {owner} {root64} {operation}' -Verb RunAs -WindowStyle Hidden -PassThru | Select-Object -ExpandProperty Id",super::network::runtime::quote(&executable.to_string_lossy()));
        let pid=match super::network::runtime::launch_detached(&root,&launch,180) {
            Ok(value)=>value,
            Err(error)=>{if !error.contains("超时") {let _=fs::remove_file(job.join("pending"));}return Err(format!("管理员授权或启动失败：{error}"));}
        };
        fs::write(job.join("pid"),pid.trim()).map_err(|e|e.to_string())?;
        let start=Instant::now();
        loop {
            if job.join("result.json").exists() {
                let data=fs::read(job.join("result.json")).map_err(|e|e.to_string())?;
                let _=fs::remove_file(job.join("pending"));
                return serde_json::from_slice(&data).map_err(|e|e.to_string());
            }
            if start.elapsed()>Duration::from_secs(1800) { return Err("系统修复仍在进行，记录已保留，请勿重复运行".into()); }
            std::thread::sleep(Duration::from_millis(500));
        }
    }).await.map_err(|e|e.to_string())?
}
pub fn helper_entry() -> bool {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) != Some("--winease-system-repair") {
        return false;
    }
    if args.len() != 6
        || uuid::Uuid::parse_str(&args[2]).is_err()
        || !(crate::presets::valid_operation(&args[5])
            || matches!(
                args[5].as_str(),
                "system" | "update-services" | "restore-update-services"
            ))
    {
        return true;
    }
    let run = || -> Result<(), String> {
        let decoded = STANDARD.decode(&args[4]).map_err(|e| e.to_string())?;
        let root = std::path::PathBuf::from(String::from_utf8(decoded).map_err(|e| e.to_string())?);
        if !root.is_absolute()
            || !root.ends_with(r"WinToolbox\SystemRepair")
            || !crate::cleaning::no_reparse_ancestors(&root)
        {
            return Err("修复任务路径无效".into());
        }
        let job = root.join(&args[2]);
        if !job.join("pending").is_file() || !crate::cleaning::no_reparse_ancestors(&job) {
            return Err("无效修复任务".into());
        }
        let body = if crate::presets::valid_operation(&args[5]) {
            crate::presets::helper_script(&args[5])?
        } else if args[5] == "system" {
            REPAIR.to_string()
        } else {
            UPDATE_SERVICES.to_string()
        };
        let prefix = format!(
            "$jobRoot='{}';$operation='{}'\n",
            super::network::runtime::quote(&root.to_string_lossy()),
            args[5]
        );
        let script=format!("{prefix}if([Security.Principal.WindowsIdentity]::GetCurrent().User.Value -ne '{}'){{@{{success=$false;message='请使用当前 Windows 账户授权修复';logs='';restart=$false}}|ConvertTo-Json -Compress;exit}}\n{body}",super::network::runtime::quote(&args[3]));
        let result = super::network::runtime::run(&root, &script, 1800)
            .and_then(|raw| serde_json::from_str::<RepairResult>(&raw).map_err(|e| e.to_string()))
            .unwrap_or_else(|error| RepairResult {
                success: false,
                message: error,
                logs: String::new(),
                restart: false,
            });
        fs::write(
            job.join("result.tmp"),
            serde_json::to_vec(&result).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        fs::rename(job.join("result.tmp"), job.join("result.json")).map_err(|e| e.to_string())
    };
    let _ = run();
    true
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(dism_code: u32, sfc_code: u32) -> RepairResult {
        let root = tempfile::tempdir().unwrap();
        let script = REPAIR
            .replace(
                r#"& "$env:SystemRoot\System32\dism.exe""#,
                "Invoke-TestDism",
            )
            .replace(r#"& "$env:SystemRoot\System32\sfc.exe""#, "Invoke-TestSfc");
        let body=format!("function Invoke-TestDism {{$global:LASTEXITCODE={dism_code};'fixture DISM'}}\nfunction Invoke-TestSfc {{$global:LASTEXITCODE={sfc_code};'fixture SFC'}}\n{script}");
        serde_json::from_str(&super::super::network::runtime::run(root.path(), &body, 10).unwrap())
            .unwrap()
    }
    #[test]
    fn update_service_repair_and_restore_keep_original_configuration() {
        let root = tempfile::tempdir().unwrap();
        let body =
            UPDATE_SERVICES.replace(r#"& "$env:windir\System32\sc.exe""#, "Invoke-FixtureSc");
        let setup = r#"
$global:services=@{wuauserv=@{StartMode='Disabled';State='Stopped'};BITS=@{StartMode='Manual';State='Stopped'}}
function Test-Path {param($Path,$LiteralPath) if($Path -like 'HKLM:*'){return $false};if($LiteralPath){Microsoft.PowerShell.Management\Test-Path -LiteralPath $LiteralPath}else{Microsoft.PowerShell.Management\Test-Path $Path}}
function Get-CimInstance {param($Class,$Filter,$OperationTimeoutSec) $name=($Filter -split "'")[1];[pscustomobject]$global:services[$name]}
function Invoke-FixtureSc {param($cmd,$name,$start,$mode) $global:services[$name].StartMode=if($mode -eq 'disabled'){'Disabled'}else{'Manual'};$global:LASTEXITCODE=0;'fixture config'}
function Start-Service {param($Name,$ErrorAction) $global:services[$Name].State='Running'}
function Stop-Service {param($Name,$ErrorAction) $global:services[$Name].State='Stopped'}
"#;
        let script=format!("$jobRoot='{}'\n{setup}\n$operation='update-services';$first=&{{{body}}}\n$operation='restore-update-services';$second=&{{{body}}}\n@{{first=($first|ConvertFrom-Json);second=($second|ConvertFrom-Json);services=$global:services}}|ConvertTo-Json -Depth 6 -Compress",crate::network::runtime::quote(&root.path().to_string_lossy()));
        let value: serde_json::Value =
            serde_json::from_str(&crate::network::runtime::run(root.path(), &script, 15).unwrap())
                .unwrap();
        assert_eq!(value["first"]["success"], true);
        assert_eq!(value["second"]["success"], true);
        assert_eq!(value["services"]["wuauserv"]["StartMode"], "Disabled");
        assert_eq!(value["services"]["wuauserv"]["State"], "Stopped");
        assert_eq!(value["services"]["BITS"]["StartMode"], "Manual");
        assert!(!root.path().join("update-service-backup.json").exists());
    }
    #[test]
    fn service_restore_rejects_unreviewed_names_before_any_change() {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("update-service-backup.json"),
            r#"[{"name":"WinDefend","mode":"Disabled","state":"Stopped"}]"#,
        )
        .unwrap();
        let setup = r#"function Test-Path {param($Path,$LiteralPath) if($Path -like 'HKLM:*'){return $false};if($LiteralPath){Microsoft.PowerShell.Management\Test-Path -LiteralPath $LiteralPath}else{Microsoft.PowerShell.Management\Test-Path $Path}}
function Stop-Service {throw 'unexpected mutation'}"#;
        let script = format!(
            "$jobRoot='{}';$operation='restore-update-services'\n{setup}\n{UPDATE_SERVICES}",
            crate::network::runtime::quote(&root.path().to_string_lossy())
        );
        let result: RepairResult =
            serde_json::from_str(&crate::network::runtime::run(root.path(), &script, 15).unwrap())
                .unwrap();
        assert!(!result.success);
        assert!(result.message.contains("恢复记录无效"));
        assert!(root.path().join("update-service-backup.json").exists());
    }
    #[test]
    fn failed_component_repair_does_not_run_system_file_check() {
        let result = fixture(5, 0);
        assert!(!result.success);
        assert!(!result.logs.contains("fixture SFC"));
    }
    #[test]
    fn restart_and_file_check_failure_are_preserved() {
        let result = fixture(3010, 1);
        assert!(!result.success);
        assert!(result.restart);
        assert!(result.logs.contains("fixture SFC"));
    }
}

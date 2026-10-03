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
#[tauri::command]
pub async fn repair_windows() -> Result<RepairResult, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let _lock=LOCK.try_lock().map_err(|_|"系统修复正在进行")?;
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
        let launch=format!("$ErrorActionPreference='Stop';Start-Process -FilePath '{}' -ArgumentList '--winease-system-repair {id} {owner} {root64}' -Verb RunAs -WindowStyle Hidden -PassThru | Select-Object -ExpandProperty Id",super::network::runtime::quote(&executable.to_string_lossy()));
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
    if args.len() != 5 || uuid::Uuid::parse_str(&args[2]).is_err() {
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
        let script=format!("if([Security.Principal.WindowsIdentity]::GetCurrent().User.Value -ne '{}'){{@{{success=$false;message='请使用当前 Windows 账户授权修复';logs='';restart=$false}}|ConvertTo-Json -Compress;exit}}\n{REPAIR}",super::network::runtime::quote(&args[3]));
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

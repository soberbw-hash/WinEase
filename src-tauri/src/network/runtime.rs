use base64::{engine::general_purpose::STANDARD, Engine};
use std::{
    fs::{self, File},
    io::Write,
    os::windows::io::AsRawHandle,
    os::windows::process::CommandExt,
    path::Path,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use wait_timeout::ChildExt;
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::{
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
                SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            },
            Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
        },
    },
};
struct Job(HANDLE);
impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
impl Job {
    fn assign(child: &std::process::Child) -> Result<Self, String> {
        unsafe {
            let job = Self(CreateJobObjectW(None, PCWSTR::null()).map_err(|e| e.to_string())?);
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&info) as u32,
            )
            .map_err(|e| e.to_string())?;
            AssignProcessToJobObject(job.0, HANDLE(child.as_raw_handle()))
                .map_err(|e| e.to_string())?;
            Ok(job)
        }
    }
}

pub fn quote(value: &str) -> String {
    value.replace('\'', "''")
}

// Input is assembled only from embedded scripts and backend-validated data, never renderer code.
// Pipe scripts into PowerShell instead of elevating a mutable .ps1 file in a user-writable directory.
pub fn run(root: &Path, body: &str, seconds: u64) -> Result<String, String> {
    run_inner(root, body, seconds, true, None)
}
pub fn launch_detached(root: &Path, body: &str, seconds: u64) -> Result<String, String> {
    run_inner(root, body, seconds, false, None)
}
pub fn run_cancellable(
    root: &Path,
    body: &str,
    seconds: u64,
    cancel: &AtomicBool,
) -> Result<String, String> {
    run_inner(root, body, seconds, true, Some(cancel))
}
fn run_inner(
    root: &Path,
    body: &str,
    seconds: u64,
    protect_children: bool,
    cancel: Option<&AtomicBool>,
) -> Result<String, String> {
    let directory = root.join("jobs").join(uuid::Uuid::new_v4().to_string());
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    if !crate::cleaning::no_reparse_ancestors(&directory) {
        return Err("网络任务目录包含链接，已停止操作".into());
    }
    let output = File::create(directory.join("stdout")).map_err(|e| e.to_string())?;
    let error = File::create(directory.join("stderr")).map_err(|e| e.to_string())?;
    let mut child = Command::new(super::powershell_path())
        .args(["-NoLogo","-NoProfile","-NonInteractive","-ExecutionPolicy","Bypass","-Command", "$ErrorActionPreference='Stop';[Console]::InputEncoding=$OutputEncoding=[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false);$code=[Console]::In.ReadToEnd(); & ([scriptblock]::Create($code))"])
        .stdin(Stdio::piped()).stdout(output).stderr(error).creation_flags(0x08000000)
        .spawn().map_err(|e| e.to_string())?;
    // Attach before feeding any script, so a timeout terminates its native children too.
    let _job = if protect_children {
        Some(match Job::assign(&child) {
            Ok(job) => job,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("无法建立网络命令的超时保护：{error}"));
            }
        })
    } else {
        None
    };
    let write_result = child
        .stdin
        .take()
        .ok_or("无法写入网络任务")?
        .write_all(body.as_bytes());
    if let Err(e) = write_result {
        let _ = child.kill();
        let _ = child.wait();
        return Err(e.to_string());
    }
    let start = Instant::now();
    let status = loop {
        if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = fs::remove_dir_all(&directory);
            return Err("测速已停止".into());
        }
        if start.elapsed() >= Duration::from_secs(seconds) {
            break None;
        }
        if let Some(status) = child
            .wait_timeout(Duration::from_millis(100))
            .map_err(|e| e.to_string())?
        {
            break Some(status);
        }
    };
    let Some(status) = status else {
        let _ = child.kill();
        let _ = child.wait();
        let _ = fs::remove_dir_all(&directory);
        return Err("网络检查或命令超时，已终止；请重新检查当前状态。".into());
    };
    let stdout = fs::read_to_string(directory.join("stdout")).unwrap_or_default();
    let stderr = fs::read_to_string(directory.join("stderr")).unwrap_or_default();
    let _ = fs::remove_dir_all(&directory);
    if !status.success() {
        return Err(if stderr.trim().is_empty() {
            format!("网络命令失败（{}）", status.code().unwrap_or(-1))
        } else {
            stderr.trim().to_string()
        });
    }
    Ok(stdout.trim_start_matches('\u{feff}').trim().to_string())
}

pub fn elevated(
    root: &Path,
    operation: &str,
    action: &str,
    sid: &str,
    snapshot: &str,
) -> Result<super::Outcome, String> {
    let job = uuid::Uuid::new_v4().to_string();
    let directory = root.join("admin-jobs").join(&job);
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    if !crate::cleaning::no_reparse_ancestors(&directory) {
        return Err("管理员任务目录包含链接".into());
    }
    fs::write(directory.join("pending"), b"pending").map_err(|e| e.to_string())?;
    let args = [
        "--winease-network-helper",
        operation,
        action,
        sid,
        root.to_str().ok_or("路径无效")?,
        &job,
        snapshot,
    ];
    // Use a base64 JSON argument array to avoid PowerShell/native quoting ambiguities.
    let args_json = STANDARD.encode(serde_json::to_vec(&args).map_err(|e| e.to_string())?);
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let launch = format!(
        r#"
$ErrorActionPreference='Stop'
$argsJson=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{args_json}'))
$list=@($argsJson|ConvertFrom-Json)
$quoted=($list|ForEach-Object {{ '"'+ $_.Replace('"','\"') +'"' }}) -join ' '
$p=Start-Process -FilePath '{}' -ArgumentList $quoted -Verb RunAs -WindowStyle Hidden -PassThru
$p.Id
"#,
        quote(&executable.to_string_lossy())
    );
    // The short-lived launcher must not own the elevated helper's lifetime.
    // The helper puts its actual PowerShell/native repair commands in a protected Job itself.
    let pid = match run_inner(root, &launch, 180, false, None) {
        Ok(value) => value,
        Err(error) => {
            if !error.contains("超时") {
                let _ = fs::remove_file(directory.join("pending"));
            }
            return Err(format!("管理员授权取消或启动失败：{error}"));
        }
    };
    let pid: u32 = pid
        .trim()
        .parse()
        .map_err(|_| "管理员任务没有返回进程编号")?;
    fs::write(directory.join("pid"), pid.to_string()).map_err(|e| e.to_string())?;
    for _ in 0..600 {
        if directory.join("result.json").is_file() {
            let raw = fs::read(directory.join("result.json")).map_err(|e| e.to_string())?;
            let outcome =
                serde_json::from_slice(&raw).map_err(|e| format!("管理员操作结果无效：{e}"))?;
            let _ = fs::remove_file(directory.join("pending"));
            return Ok(outcome);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Err(
        "管理员操作尚未返回结果。备份和任务记录已保留，请等待操作结束后再检查网络；不要重复修复。"
            .into(),
    )
}

pub fn pending(root: &Path) -> bool {
    fs::read_dir(root.join("admin-jobs")).is_ok_and(|entries| {
        entries.flatten().any(|e| {
            let path = e.path();
            if !path.join("pending").exists() || path.join("result.json").exists() {
                return false;
            }
            let pid = fs::read_to_string(path.join("pid"))
                .ok()
                .and_then(|s| s.parse::<u32>().ok());
            // A dead helper must not block repair forever. Unknown/access-denied processes stay protected.
            if let Some(pid) = pid {
                unsafe {
                    if let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                        let mut exit = 259;
                        let known = GetExitCodeProcess(handle, &mut exit).is_ok();
                        let _ = CloseHandle(handle);
                        if known && exit != 259 {
                            let _ = fs::remove_file(path.join("pending"));
                            return false;
                        }
                    } else if windows::core::Error::from_thread().code().0 as u32 == 0x80070057 {
                        let _ = fs::remove_file(path.join("pending"));
                        return false;
                    }
                }
            }
            true
        })
    })
}

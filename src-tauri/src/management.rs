use crate::{format_process_details, run_command_capture, run_powershell_json};
use serde_json::{json, Value};
use std::sync::Mutex;
use winreg::{enums::*, RegKey};

static STARTUP_LOCK: Mutex<()> = Mutex::new(());
const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const BACKUP: &str = r"Software\WinToolbox\DisabledStartup";
const WINDOW_API: &str = r#"
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class ToolboxWindows {
 public delegate bool EnumCallback(IntPtr hwnd, IntPtr param);
 [DllImport("user32.dll")] static extern bool EnumWindows(EnumCallback cb, IntPtr param);
 [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hwnd);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int length);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
 [DllImport("user32.dll")] public static extern IntPtr SendMessageTimeout(IntPtr hwnd, uint msg, IntPtr wparam, IntPtr lparam, uint flags, uint timeout, out IntPtr result);
 public class Window { public long windowId; public uint pid; public string title; }
 public static Window[] List() {
  var rows=new List<Window>();
  EnumWindows((hwnd,param)=>{if(IsWindowVisible(hwnd)){var text=new StringBuilder(1024);GetWindowText(hwnd,text,1024);if(text.Length>0){uint pid;GetWindowThreadProcessId(hwnd,out pid);rows.Add(new Window{windowId=hwnd.ToInt64(),pid=pid,title=text.ToString()});}}return true;},IntPtr.Zero);
  return rows.ToArray();
 }
}
'@
"#;

#[tauri::command]
pub async fn list_application_windows() -> Result<Value, String> {
    json_script(format!(r#"{WINDOW_API}
$me=Get-Process -Id {}
$rows=@([ToolboxWindows]::List() | ForEach-Object {{
 try {{
  $p=Get-Process -Id $_.pid -ErrorAction Stop
  if($p.SessionId -eq $me.SessionId) {{
   $path=$p.Path; $protected=(!$path -or $path.StartsWith($env:windir,[StringComparison]::OrdinalIgnoreCase) -or $p.Id -eq $me.Id -or $p.ProcessName -match '^(explorer|winease|win-toolbox|powershell|pwsh|dwm|csrss|winlogon|services|lsass|svchost|sihost|fontdrvhost)$')
   [pscustomobject]@{{pid=$p.Id;name=$p.ProcessName;memoryBytes=$p.WorkingSet64;path=$path;stamp=[string]$p.StartTime.ToUniversalTime().Ticks;title=$_.title;windowId=[string]$_.windowId;canEnd=!$protected}}
  }}
 }} catch {{}}
}})
ConvertTo-Json -InputObject $rows -Depth 4 -Compress
"#,std::process::id())).await
}

fn ps_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
async fn json_script(script: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        serde_json::from_str(&run_powershell_json(&script)?).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn list_processes() -> Result<Value, String> {
    json_script(format!(r#"
$ErrorActionPreference = 'Stop'
$me = Get-Process -Id {}
$rows = @(Get-Process | Where-Object {{ $_.SessionId -eq $me.SessionId }} | ForEach-Object {{
 try {{
  $path = $_.Path; $stamp = [string]$_.StartTime.ToUniversalTime().Ticks
  $protected = (!$path -or $path.StartsWith($env:windir, [StringComparison]::OrdinalIgnoreCase) -or $_.Id -eq $me.Id -or $_.ProcessName -match '^(explorer|winease|win-toolbox|powershell|pwsh|dwm|csrss|winlogon|services|lsass|svchost|sihost|fontdrvhost)$')
  [pscustomobject]@{{ pid = $_.Id; name = $_.ProcessName; memoryBytes = $_.WorkingSet64; path = $path; stamp = $stamp; title = [string]$_.MainWindowTitle; canEnd = !$protected }}
 }} catch {{}}
}} | Sort-Object memoryBytes -Descending)
ConvertTo-Json -InputObject $rows -Depth 4 -Compress
"#, std::process::id())).await
}

#[tauri::command]
pub async fn end_process(
    pid: u32,
    stamp: String,
    close_window: bool,
    window_id: Option<String>,
) -> Result<(), String> {
    if pid <= 4
        || pid == std::process::id()
        || !stamp.chars().all(|c| c.is_ascii_digit())
        || stamp.is_empty()
    {
        return Err("不能操作该进程。".into());
    }
    let close = if close_window {
        let id = window_id
            .ok_or("请刷新窗口列表")?
            .parse::<u64>()
            .map_err(|_| "窗口标识无效")?;
        if id == 0 || id > i64::MAX as u64 {
            return Err("窗口标识无效".into());
        }
        format!(
            r#"{WINDOW_API}
$hwnd=[IntPtr][long]{id};[uint32]$owner=0
[void][ToolboxWindows]::GetWindowThreadProcessId($hwnd,[ref]$owner)
if($owner -ne $p.Id){{throw '窗口已变化，请刷新列表'}}
$result=[IntPtr]::Zero
if([ToolboxWindows]::SendMessageTimeout($hwnd,0x10,[IntPtr]::Zero,[IntPtr]::Zero,2,2000,[ref]$result) -eq [IntPtr]::Zero){{throw '窗口无响应，未能正常关闭'}}
"#
        )
    } else {
        "Stop-Process -InputObject $p -ErrorAction Stop".into()
    };
    let result = json_script(format!(r#"
$ErrorActionPreference = 'Stop'
$p = Get-Process -Id {pid}
$me = Get-Process -Id {}
if ([string]$p.StartTime.ToUniversalTime().Ticks -ne {} -or $p.SessionId -ne $me.SessionId -or !$p.Path -or $p.Path.StartsWith($env:windir,[StringComparison]::OrdinalIgnoreCase) -or $p.ProcessName -match '^(explorer|winease|win-toolbox|powershell|pwsh|dwm|csrss|winlogon|services|lsass|svchost|sihost|fontdrvhost)$') {{ throw '进程已变化或受到保护，请刷新列表' }}
{}
'true'
"#, std::process::id(), ps_string(&stamp), close)).await?;
    if result != json!(true) {
        return Err("未能完成操作。".into());
    }
    Ok(())
}

fn startup_rows_at(user: &RegKey) -> Result<Value, String> {
    let mut rows = Vec::new();
    for (path, enabled) in [(RUN, true), (BACKUP, false)] {
        if let Ok(key) = user.open_subkey(path) {
            for (name, value) in key.enum_values().flatten() {
                if value.vtype != REG_SZ && value.vtype != REG_EXPAND_SZ {
                    continue;
                }
                let command: String = key.get_value(&name).map_err(|e| e.to_string())?;
                let windows_disabled=enabled && user.open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run").ok().and_then(|key|key.get_raw_value(&name).ok()).is_some_and(|value|value.bytes.first().is_some_and(|first|matches!(first,3|7)));
                rows.push(json!({"name":name,"command":command,"enabled":enabled && !windows_disabled,"editable":!windows_disabled,"source":if windows_disabled{"Windows 已停用"}else{"当前用户"}}));
            }
        }
    }
    Ok(Value::Array(rows))
}
#[tauri::command]
pub async fn list_startup_items() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let _lock = STARTUP_LOCK.lock().map_err(|e| e.to_string())?;
        let mut rows = startup_rows_at(&RegKey::predef(HKEY_CURRENT_USER))?;
        let machine = RegKey::predef(HKEY_LOCAL_MACHINE);
        for path in [RUN, r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Run"] {
            if let Ok(key) = machine.open_subkey(path) {
                for (name, value) in key.enum_values().flatten() {
                    if value.vtype != REG_SZ && value.vtype != REG_EXPAND_SZ { continue; }
                    if let Ok(command) = key.get_value::<String, _>(&name) {
                        rows.as_array_mut().unwrap().push(json!({"name":name,"command":command,"enabled":true,"editable":false,"source":"所有用户"}));
                    }
                }
            }
        }
        Ok(rows)
    }).await.map_err(|e| e.to_string())?
}
fn change_startup_at(
    user: &RegKey,
    name: &str,
    command: &str,
    enabled: bool,
) -> Result<(), String> {
    if name.is_empty() || name.contains('\0') || name.len() > 256 {
        return Err("启动项名称无效".into());
    }
    let (source, target) = if enabled {
        (BACKUP, RUN)
    } else {
        (RUN, BACKUP)
    };
    let from = user
        .open_subkey_with_flags(source, KEY_READ | KEY_WRITE)
        .map_err(|e| e.to_string())?;
    let original = from.get_raw_value(name).map_err(|e| e.to_string())?;
    if !matches!(original.vtype, REG_SZ | REG_EXPAND_SZ)
        || from
            .get_value::<String, _>(name)
            .map_err(|e| e.to_string())?
            != command
    {
        return Err("启动项已变化，请刷新列表。".into());
    }
    let (to, _) = user.create_subkey(target).map_err(|e| e.to_string())?;
    if to.get_raw_value(name).is_ok() {
        return Err("目标位置已存在同名启动项，已停止操作。".into());
    }
    to.set_raw_value(name, &original)
        .map_err(|e| e.to_string())?;
    if let Err(e) = from.delete_value(name) {
        let _ = to.delete_value(name);
        return Err(e.to_string());
    }
    Ok(())
}
#[tauri::command]
pub async fn set_startup_item(name: String, command: String, enabled: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _lock = STARTUP_LOCK.lock().map_err(|e| e.to_string())?;
        change_startup_at(&RegKey::predef(HKEY_CURRENT_USER), &name, &command, enabled)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn health_check() -> Result<Value, String> {
    json_script(r#"
$ErrorActionPreference = 'Stop'
$os=Get-CimInstance Win32_OperatingSystem
$drives=@(Get-CimInstance Win32_LogicalDisk -Filter 'DriveType=3' | ForEach-Object { [pscustomobject]@{ name=$_.DeviceID; freeBytes=[uint64]$_.FreeSpace; totalBytes=[uint64]$_.Size } })
$reboot=(Test-Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\RebootPending') -or (Test-Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\WindowsUpdate\Auto Update\RebootRequired')
$security='未知'; try { $m=Get-MpComputerStatus -ErrorAction Stop; $security=if($m.AntivirusEnabled -and $m.RealTimeProtectionEnabled){'实时防护已开启'}else{'实时防护未开启或由其他软件接管'} } catch { $security='无法读取 Defender 状态' }
[pscustomobject]@{ memoryPercent=[math]::Round((1-$os.FreePhysicalMemory/$os.TotalVisibleMemorySize)*100); drives=$drives; pendingReboot=$reboot; security=$security; checkedAt=(Get-Date).ToString('o') } | ConvertTo-Json -Depth 5 -Compress
"#.into()).await
}
#[tauri::command]
pub async fn get_power_plan() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let active = run_command_capture("powercfg.exe", &["/getactivescheme"])?;
        if !active.success { return Err(format_process_details(&active)); }
        let output=active.stdout.to_lowercase();
        let mode=if output.contains(crate::BALANCED_GUID){"balanced"}else if output.contains(crate::HIGH_PERFORMANCE_GUID){"performance"}else{"custom"};
        let plans=run_command_capture("powercfg.exe", &["/list"])?;
        Ok(json!({"mode":mode,"performanceAvailable":plans.success && plans.stdout.to_lowercase().contains(crate::HIGH_PERFORMANCE_GUID)}))
    }).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn set_power_plan(mode: String) -> Result<(), String> {
    let guid = match mode.as_str() {
        "balanced" => crate::BALANCED_GUID,
        "performance" => crate::HIGH_PERFORMANCE_GUID,
        _ => return Err("不支持的电源计划".into()),
    };
    tauri::async_runtime::spawn_blocking(move || {
        let result = run_command_capture("powercfg.exe", &["/setactive", guid])?;
        if result.success {
            Ok(())
        } else {
            Err(format_process_details(&result))
        }
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn repair_taskbar() -> Result<(), String> {
    json_script(EXPLORER_RESTART.into()).await?;
    Ok(())
}
pub(crate) const EXPLORER_RESTART: &str = r#"
$ErrorActionPreference='Stop'
$session=(Get-Process -Id $PID).SessionId
$shell=@(Get-Process explorer -ErrorAction SilentlyContinue | Where-Object SessionId -eq $session)
if (!$shell.Count) { throw '未找到当前会话的资源管理器' }
$shell | Stop-Process -ErrorAction Stop
Start-Sleep -Milliseconds 700
if (!(Get-Process explorer -ErrorAction SilentlyContinue | Where-Object SessionId -eq $session)) { Start-Process -FilePath "$env:windir\explorer.exe" }
for($attempt=0;$attempt -lt 20;$attempt++) {
 if(Get-Process explorer -ErrorAction SilentlyContinue | Where-Object SessionId -eq $session){break}
 Start-Sleep -Milliseconds 250
}
if(!(Get-Process explorer -ErrorAction SilentlyContinue | Where-Object SessionId -eq $session)){throw '资源管理器未重新启动'}
'true'
"#;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn powershell_quote_is_literal() {
        assert_eq!(ps_string("a'; Stop-Process; '"), "'a''; Stop-Process; '''");
    }
    #[test]
    fn startup_roundtrip_preserves_kind_and_rejects_changed_entry() {
        let parent = RegKey::predef(HKEY_CURRENT_USER);
        let path = format!(r"Software\WinToolbox\Tests\startup-{}", std::process::id());
        let (user, _) = parent.create_subkey(&path).unwrap();
        let (run, _) = user.create_subkey(RUN).unwrap();
        run.set_value("Test", &"%LOCALAPPDATA%\\test.exe").unwrap();
        assert!(change_startup_at(&user, "Test", "changed", false).is_err());
        change_startup_at(&user, "Test", "%LOCALAPPDATA%\\test.exe", false).unwrap();
        assert!(run.get_value::<String, _>("Test").is_err());
        change_startup_at(&user, "Test", "%LOCALAPPDATA%\\test.exe", true).unwrap();
        assert_eq!(
            run.get_value::<String, _>("Test").unwrap(),
            "%LOCALAPPDATA%\\test.exe"
        );
        drop(run);
        drop(user);
        parent.delete_subkey_all(path).unwrap();
    }
}

use crate::{format_process_details, local_app_data_dir, run_command_capture, run_powershell_json};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

const REGISTRY_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
const DOTNET_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced";
static SETTINGS_LOCK: Mutex<()> = Mutex::new(());
struct Definition {
    id: &'static str,
    label: &'static str,
    name: &'static str,
    on: u32,
    off: u32,
}
const DEFINITIONS: [Definition; 1] = [Definition {
    id: "hidden-files",
    label: "显示隐藏文件",
    name: "Hidden",
    on: 1,
    off: 2,
}];
fn definition(id: &str) -> Result<&'static Definition, String> {
    DEFINITIONS
        .iter()
        .find(|item| item.id == id)
        .ok_or_else(|| "不支持的 Windows 设置。".into())
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsSetting {
    id: &'static str,
    label: &'static str,
    enabled: bool,
    can_restore: bool,
}
#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Backup {
    id: String,
    original: Option<u32>,
}
fn backup_path(item: &Definition) -> Result<PathBuf, String> {
    Ok(local_app_data_dir()
        .ok_or("无法定位设置备份目录。")?
        .join("WinToolbox")
        .join("SettingBackups")
        .join(format!("{}.json", item.id)))
}
fn read_value(item: &Definition) -> Result<Option<u32>, String> {
    read_value_at(item, DOTNET_KEY)
}
fn read_value_at(item: &Definition, dotnet_key: &str) -> Result<Option<u32>, String> {
    let script = format!(
        r#"
$ErrorActionPreference = 'Stop'
$key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('{dotnet_key}', $false)
try {{
  $value = if ($key) {{ $key.GetValue('{}', $null) }} else {{ $null }}
  if ($null -ne $value -and $key.GetValueKind('{}') -ne [Microsoft.Win32.RegistryValueKind]::DWord) {{ throw '设置值类型异常，已停止操作' }}
  if ($null -eq $value) {{ 'null' }} else {{ [uint32]$value | ConvertTo-Json -Compress }}
}} finally {{ if ($key) {{ $key.Dispose() }} }}
"#,
        item.name, item.name
    );
    serde_json::from_str(&run_powershell_json(&script)?).map_err(|e| format!("读取设置失败：{e}"))
}
fn ensure_backup(path: &Path, item: &Definition, original: Option<u32>) -> Result<(), String> {
    if path.exists() {
        return read_backup(path, item).map(|_| ());
    }
    fs::create_dir_all(path.parent().ok_or("备份路径异常")?).map_err(|e| e.to_string())?;
    let data = serde_json::to_vec_pretty(&Backup {
        id: item.id.into(),
        original,
    })
    .map_err(|e| e.to_string())?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("无法备份原设置：{e}"))?;
    if let Err(error) = file.write_all(&data).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(format!("设置备份未完成：{error}"));
    }
    Ok(())
}
fn read_backup(path: &Path, item: &Definition) -> Result<Backup, String> {
    let backup: Backup = serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("设置备份损坏：{e}"))?;
    if backup.id != item.id {
        return Err("设置备份不匹配。".into());
    }
    Ok(backup)
}
fn write_value(item: &Definition, value: Option<u32>) -> Result<(), String> {
    write_value_at(item, value, REGISTRY_KEY, DOTNET_KEY)
}
fn write_value_at(
    item: &Definition,
    value: Option<u32>,
    registry_key: &str,
    dotnet_key: &str,
) -> Result<(), String> {
    let capture = match value {
        Some(value) => run_command_capture(
            "reg.exe",
            &[
                "add",
                registry_key,
                "/v",
                item.name,
                "/t",
                "REG_DWORD",
                "/d",
                &value.to_string(),
                "/f",
            ],
        ),
        None => {
            if read_value_at(item, dotnet_key)?.is_none() {
                return Ok(());
            }
            run_command_capture("reg.exe", &["delete", registry_key, "/v", item.name, "/f"])
        }
    }?;
    if !capture.success {
        return Err(format_process_details(&capture));
    }
    if read_value_at(item, dotnet_key)? != value {
        return Err("设置写入后校验失败，原设置备份已保留。".into());
    }
    Ok(())
}
#[tauri::command]
pub async fn get_windows_settings() -> Result<Vec<WindowsSetting>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let _guard = SETTINGS_LOCK.lock().map_err(|_| "设置任务状态异常")?;
        DEFINITIONS
            .iter()
            .map(|item| {
                let value = read_value(item)?.unwrap_or(item.off);
                let path = backup_path(item)?;
                let can_restore = if path.exists() {
                    read_backup(&path, item)?;
                    true
                } else {
                    false
                };
                Ok(WindowsSetting {
                    id: item.id,
                    label: item.label,
                    enabled: value == item.on,
                    can_restore,
                })
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn set_windows_setting(id: String, enabled: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = SETTINGS_LOCK.lock().map_err(|_| "设置任务状态异常")?;
        let item = definition(&id)?;
        let original = read_value(item)?;
        ensure_backup(&backup_path(item)?, item, original)?;
        write_value(item, Some(if enabled { item.on } else { item.off }))?;
        refresh_explorer()
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn restore_windows_setting(id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = SETTINGS_LOCK.lock().map_err(|_| "设置任务状态异常")?;
        let item = definition(&id)?;
        let path = backup_path(item)?;
        let backup = read_backup(&path, item)?;
        write_value(item, backup.original)?;
        refresh_explorer()?;
        fs::remove_file(path).map_err(|e| format!("已恢复，备份记录清除失败：{e}"))
    })
    .await
    .map_err(|e| e.to_string())?
}
fn refresh_explorer() -> Result<(), String> {
    let capture = run_command_capture(
        "powershell.exe",
        &[
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            crate::management::EXPLORER_RESTART,
        ],
    )?;
    if capture.success {
        Ok(())
    } else {
        Err(format!(
            "设置已保存，但资源管理器刷新失败：{}",
            format_process_details(&capture)
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_registry_round_trip_uses_isolated_key() {
        let dotnet_key = format!(
            r"Software\WinToolbox\Tests\round-trip-{}",
            std::process::id()
        );
        let registry_key = format!("HKCU\\{dotnet_key}");
        let folder = tempfile::tempdir().unwrap();
        for item in &DEFINITIONS {
            let path = folder.path().join(format!("{}.json", item.id));
            assert_eq!(read_value_at(item, &dotnet_key).unwrap(), None);
            ensure_backup(&path, item, None).unwrap();
            write_value_at(item, Some(item.on), &registry_key, &dotnet_key).unwrap();
            assert_eq!(read_value_at(item, &dotnet_key).unwrap(), Some(item.on));
            write_value_at(item, Some(item.off), &registry_key, &dotnet_key).unwrap();
            ensure_backup(&path, item, Some(item.off)).unwrap();
            write_value_at(
                item,
                read_backup(&path, item).unwrap().original,
                &registry_key,
                &dotnet_key,
            )
            .unwrap();
            assert_eq!(read_value_at(item, &dotnet_key).unwrap(), None);
        }
        assert!(registry_key.starts_with(r"HKCU\Software\WinToolbox\Tests\round-trip-"));
        assert!(
            run_command_capture("reg.exe", &["delete", &registry_key, "/f"])
                .unwrap()
                .success
        );
    }
    #[test]
    fn repeated_changes_preserve_first_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("backup.json");
        let item = definition("hidden-files").unwrap();
        ensure_backup(&path, item, Some(2)).unwrap();
        ensure_backup(&path, item, Some(1)).unwrap();
        assert_eq!(read_backup(&path, item).unwrap().original, Some(2));
    }
    #[test]
    fn originally_absent_value_is_recorded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("backup.json");
        let item = definition("hidden-files").unwrap();
        ensure_backup(&path, item, None).unwrap();
        assert_eq!(read_backup(&path, item).unwrap().original, None);
    }
    #[test]
    fn invalid_ids_and_mismatched_backups_are_rejected() {
        assert!(definition("../hidden-files").is_err());
        assert!(definition("file-extensions").is_err());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("backup.json");
        fs::write(&path, br#"{"id":"file-extensions","original":0}"#).unwrap();
        assert!(read_backup(&path, definition("hidden-files").unwrap()).is_err());
    }
    #[test]
    fn corrupt_backup_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("backup.json");
        fs::write(&path, "broken").unwrap();
        assert!(ensure_backup(&path, definition("hidden-files").unwrap(), Some(2)).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "broken");
    }
}

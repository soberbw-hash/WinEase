use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tauri::Emitter;

const ENGINE: &str = include_str!("../../resources/network/speed.cs");
static ACTIVE: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Measurement {
    pub mbps: Option<f64>,
    pub latency_ms: Option<f64>,
    pub jitter_ms: Option<f64>,
    pub bytes: u64,
    pub requested_bytes: u64,
    pub seconds: f64,
    pub completed_requests: u32,
    pub error: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedResult {
    latency: Measurement,
    download: Measurement,
    upload: Measurement,
    checked_at: String,
    server: &'static str,
}
fn measure(root: &std::path::Path, mode: &str, cancel: &AtomicBool) -> Result<Measurement, String> {
    let (endpoint, cap) = match mode {
        "latency" => ("__down", 0),
        "download" => ("__down", 96 * 1024 * 1024),
        "upload" => ("__up", 48 * 1024 * 1024),
        _ => return Err("未知测速类型".into()),
    };
    let body = format!(
        r#"
Add-Type -TypeDefinition @'
{ENGINE}
'@ -ReferencedAssemblies 'System.Net.Http','System','System.Core'
[WinEaseSpeed]::Run('{mode}','https://speed.cloudflare.com/{endpoint}',{cap},8000,4) | ConvertTo-Json -Compress
"#
    );
    let data = super::runtime::run_cancellable(
        root,
        &body,
        if mode == "latency" { 40 } else { 20 },
        cancel,
    )?;
    serde_json::from_str(&data).map_err(|e| format!("测速结果无效：{e}"))
}
struct ActiveGuard;
impl Drop for ActiveGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = ACTIVE.lock() {
            *active = None;
        }
    }
}
#[tauri::command]
pub async fn network_speedtest(app: tauri::AppHandle) -> Result<SpeedResult, String> {
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut active = ACTIVE.lock().map_err(|_| "测速状态异常")?;
        if active.is_some() {
            return Err("测速正在进行".into());
        }
        *active = Some(cancel.clone());
    }
    let active_guard = ActiveGuard;
    tauri::async_runtime::spawn_blocking(move || {
        let _active = active_guard;
        let _lock = super::LOCK
            .try_lock()
            .map_err(|_| "网络任务正在进行，请稍后测速")?;
        let root = super::root()?;
        if super::runtime::pending(&root) {
            return Err("管理员网络操作尚未结束，请稍后测速".into());
        }
        app.emit("network-speed-stage", "latency")
            .map_err(|e| e.to_string())?;
        let latency = measure(&root, "latency", &cancel)?;
        app.emit("network-speed-stage", "download")
            .map_err(|e| e.to_string())?;
        let download = measure(&root, "download", &cancel)?;
        app.emit("network-speed-stage", "upload")
            .map_err(|e| e.to_string())?;
        let upload = measure(&root, "upload", &cancel)?;
        if cancel.load(Ordering::Relaxed) {
            return Err("测速已停止".into());
        }
        Ok(SpeedResult {
            latency,
            download,
            upload,
            checked_at: chrono::Utc::now().to_rfc3339(),
            server: "Cloudflare",
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn network_cancel_speedtest() -> Result<(), String> {
    if let Some(cancel) = ACTIVE.lock().map_err(|_| "测速状态异常")?.as_ref() {
        cancel.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_terminates_measurement_without_touching_network() {
        let directory = tempfile::tempdir().unwrap();
        let cancel = AtomicBool::new(true);
        assert!(super::super::runtime::run_cancellable(
            directory.path(),
            "Start-Sleep 10",
            20,
            &cancel
        )
        .unwrap_err()
        .contains("停止"));
    }
    #[test]
    fn unavailable_measurement_remains_null_not_zero() {
        let result: Measurement = serde_json::from_str(r#"{"mbps":null,"latencyMs":null,"jitterMs":null,"bytes":0,"requestedBytes":65536,"seconds":8.0,"completedRequests":0,"error":"timeout"}"#).unwrap();
        assert!(result.mbps.is_none());
        assert_eq!(result.error.as_deref(), Some("timeout"));
    }
    #[test]
    fn running_measurement_can_be_stopped_promptly() {
        let directory = tempfile::tempdir().unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let trigger = cancel.clone();
        let thread = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(500));
            trigger.store(true, Ordering::Relaxed);
        });
        let start = std::time::Instant::now();
        assert!(super::super::runtime::run_cancellable(
            directory.path(),
            "Start-Sleep 10",
            20,
            &cancel
        )
        .unwrap_err()
        .contains("停止"));
        thread.join().unwrap();
        assert!(start.elapsed() < std::time::Duration::from_secs(3));
    }
    #[test]
    #[ignore = "explicit bandwidth test: up to 144 MiB of generated traffic, never changes settings"]
    fn measures_real_download_and_upload() {
        let directory = tempfile::tempdir().unwrap();
        let cancel = AtomicBool::new(false);
        for mode in ["latency", "download", "upload"] {
            let result = measure(directory.path(), mode, &cancel).unwrap();
            println!("{mode}: {result:?}");
            assert!(result.error.is_none(), "{mode} failed");
            if mode != "latency" {
                assert!(result.mbps.is_some_and(|rate| rate > 0.0));
            }
        }
    }
}

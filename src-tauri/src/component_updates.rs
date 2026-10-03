use crate::{
    build_action_result, component_definitions_internal, format_process_details,
    list_components_internal, ToolActionResult,
};
use serde::Serialize;
use std::collections::HashSet;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::Instant;
static LOCK: Mutex<()> = Mutex::new(());
static TASKS: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
pub(crate) struct ComponentTask(String);
impl Drop for ComponentTask {
    fn drop(&mut self) {
        if let Ok(mut tasks) = TASKS.lock() {
            tasks.remove(&self.0);
        }
    }
}
pub(crate) fn begin_component_task(id: &str) -> Result<ComponentTask, String> {
    // This guard only reserves a task slot; each operation validates its fixed catalog ID.
    // Do not enumerate installed packages here: that would delay opening unrelated apps.
    if id.is_empty() || id.len() > 128 || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return Err("无效组件标识。".into());
    }
    let mut tasks = TASKS.lock().map_err(|e| e.to_string())?;
    if !tasks.insert(id.to_owned()) {
        return Err("此组件已有任务正在进行。".into());
    }
    Ok(ComponentTask(id.to_owned()))
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentUpdate {
    id: String,
    available: bool,
    version: Option<String>,
}
fn available_version(output: &str, id: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let cells: Vec<_> = line.split_whitespace().collect();
        let index = cells
            .iter()
            .position(|value| value.eq_ignore_ascii_case(id))?;
        let installed = *cells.get(index + 1)?;
        let available = *cells.get(index + 2)?;
        if installed == available || available == "winget" || available == "msstore" {
            None
        } else {
            Some(available.to_string())
        }
    })
}
fn run_winget(args: &[&str], seconds: u64) -> Result<crate::ProcessCapture, String> {
    let root = crate::local_app_data_dir()
        .ok_or("无法定位用户目录")?
        .join(r"WinToolbox\ComponentUpdates");
    let arguments = args
        .iter()
        .map(|arg| format!("'{}'", crate::network::runtime::quote(arg)))
        .collect::<Vec<_>>()
        .join(",");
    let body = format!(
        r#"$ErrorActionPreference='Stop';$arguments=@({arguments});$output=(& winget @arguments 2>&1 | Out-String);$exit=$LASTEXITCODE;@{{exit=$exit;output=$output}}|ConvertTo-Json -Compress"#
    );
    let output = crate::network::runtime::run(&root, &body, seconds)?;
    let value: serde_json::Value = serde_json::from_str(&output).map_err(|e| e.to_string())?;
    let code = value["exit"].as_i64().ok_or("winget 未返回退出码")? as i32;
    Ok(crate::ProcessCapture {
        exit_code: Some(code),
        success: code == 0,
        stdout: value["output"].as_str().unwrap_or_default().into(),
        stderr: String::new(),
    })
}
#[tauri::command]
pub async fn check_component_updates() -> Result<Vec<ComponentUpdate>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let _guard = LOCK.try_lock().map_err(|_| "组件更新任务正在进行")?;
        let capture = run_winget(
            &[
                "list",
                "--upgrade-available",
                "--source",
                "winget",
                "--accept-source-agreements",
                "--disable-interactivity",
            ],
            45,
        )?;
        // No installed package / no applicable upgrade are normal empty results.
        if !capture.success && !matches!(capture.exit_code, Some(-1978335212) | Some(-1978335189)) {
            return Err(format!(
                "组件更新检查失败：{}",
                format_process_details(&capture)
            ));
        }
        Ok(component_definitions_internal()
            .into_iter()
            .filter(|c| c.supports_update && c.winget_id.is_some())
            .map(|c| {
                let version = available_version(&capture.stdout, c.winget_id.unwrap());
                ComponentUpdate {
                    id: c.id.to_string(),
                    available: version.is_some(),
                    version,
                }
            })
            .collect())
    })
    .await
    .map_err(|e| e.to_string())?
}
pub fn update_component(id: &str) -> Result<ToolActionResult, String> {
    let started = Instant::now();
    let component = list_components_internal()
        .into_iter()
        .find(|c| c.id == id && c.installed && c.supports_update)
        .ok_or("该组件未安装或不支持更新")?;
    let package = component.winget_id.as_deref().ok_or("此组件没有更新来源")?;
    let capture = run_winget(
        &[
            "upgrade",
            "--id",
            package,
            "--exact",
            "--source",
            "winget",
            "--silent",
            "--accept-package-agreements",
            "--accept-source-agreements",
            "--disable-interactivity",
        ],
        900,
    )?;
    let no_upgrade = capture.exit_code == Some(-1978335189);
    Ok(build_action_result(
        "update_component",
        "更新组件",
        capture.success || no_upgrade,
        if no_upgrade {
            format!("{} 已是最新版本。", component.name)
        } else if capture.success {
            format!("{} 已更新。", component.name)
        } else {
            format!("{} 更新失败。", component.name)
        },
        format_process_details(&capture),
        None,
        Vec::new(),
        started,
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn different_components_can_run_and_same_component_is_rejected() {
        let first = begin_component_task("capture-plus").unwrap();
        let second = begin_component_task("uninstall-plus").unwrap();
        assert!(begin_component_task("capture-plus").is_err());
        drop(second);
        assert!(begin_component_task("capture-plus").is_err());
        drop(first);
        assert!(begin_component_task("capture-plus").is_ok());
        assert!(begin_component_task("../component").is_err());
    }
    #[test]
    #[ignore = "read-only winget availability check; never updates installed packages"]
    fn real_winget_output_can_be_read_with_timeout() {
        let capture = run_winget(
            &[
                "list",
                "--upgrade-available",
                "--source",
                "winget",
                "--accept-source-agreements",
                "--disable-interactivity",
            ],
            45,
        )
        .unwrap();
        assert!(
            capture.success || matches!(capture.exit_code, Some(-1978335212) | Some(-1978335189))
        );
        println!("component availability read completed without installing");
    }
    #[test]
    #[ignore = "verify generated signed installer and reject modified bytes"]
    fn release_signature_matches_pinned_key_and_rejects_tampering() {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(root.join("tauri.conf.json")).unwrap())
                .unwrap();
        let key = String::from_utf8(
            STANDARD
                .decode(config["plugins"]["updater"]["pubkey"].as_str().unwrap())
                .unwrap(),
        )
        .unwrap();
        let public = minisign_verify::PublicKey::decode(&key).unwrap();
        let installer = root.join("target/release/bundle/nsis/WinEase_3.5.0_x64-setup.exe");
        let signature = std::fs::read_to_string(installer.with_extension("exe.sig")).unwrap();
        let signature = String::from_utf8(STANDARD.decode(signature.trim()).unwrap()).unwrap();
        let signature = minisign_verify::Signature::decode(&signature).unwrap();
        let mut bytes = std::fs::read(installer).unwrap();
        public.verify(&bytes, &signature, true).unwrap();
        bytes[0] ^= 1;
        assert!(public.verify(&bytes, &signature, true).is_err());
    }
    #[test]
    fn requires_exact_id_and_reports_available_column() {
        assert_eq!(
            available_version("App Foo.Bar 1.2.0 1.3.0 winget", "Foo.Bar"),
            Some("1.3.0".into())
        );
        assert_eq!(
            available_version("App Foo.BarExtra 1.2 1.3 winget", "Foo.Bar"),
            None
        );
        assert_eq!(available_version("App Foo.Bar 1.2 winget", "Foo.Bar"), None);
    }
}

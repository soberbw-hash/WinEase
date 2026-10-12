//! Reviewed bundles: the UI supplies only a preset ID, never registry paths or commands.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeSet, fs, sync::Mutex};
static ACTIVE: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());
struct Lease(Vec<String>);
impl Lease {
    fn enter(ids: &[String]) -> Result<Self, String> {
        let mut active = ACTIVE.lock().map_err(|_| "优化任务状态异常")?;
        if ids.iter().any(|id| active.contains(id)) {
            return Err("该项优化正在进行".into());
        }
        active.extend(ids.iter().cloned());
        Ok(Self(ids.to_vec()))
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        if let Ok(mut active) = ACTIVE.lock() {
            for id in &self.0 {
                active.remove(id);
            }
        }
    }
}
const SCRIPT: &str = include_str!("../resources/optimization-presets.ps1");
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: String,
    pub label: String,
    pub description: String,
    pub enabled: Option<bool>,
    pub can_restore: bool,
    pub error: Option<String>,
}
fn field(key: &str, name: &str, on: u32, off: u32) -> Value {
    json!({"key":key,"name":name,"on":on,"off":off})
}
fn catalog() -> Vec<Value> {
    let cdm = r"HKCU\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager";
    let mut ads: Vec<Value> = [
        "SilentInstalledAppsEnabled",
        "SoftLandingEnabled",
        "SystemPaneSuggestionsEnabled",
        "SubscribedContent-338389Enabled",
        "SubscribedContent-338393Enabled",
        "SubscribedContent-353694Enabled",
        "SubscribedContent-353696Enabled",
        "SubscribedContent-353698Enabled",
    ]
    .iter()
    .map(|n| field(cdm, n, 0, 1))
    .collect();
    ads.extend([
        field(
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
            "ShowSyncProviderNotifications",
            0,
            1,
        ),
        field(
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\AdvertisingInfo",
            "Enabled",
            0,
            1,
        ),
        field(
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Privacy",
            "TailoredExperiencesWithDiagnosticDataEnabled",
            0,
            1,
        ),
    ]);
    let storage = r"HKLM\SOFTWARE\Policies\Microsoft\Windows\StorageSense";
    vec![
        json!({"id":"privacy-ads","label":"关闭广告与推荐","description":"关闭系统推广、广告标识与推荐应用安装；已有应用保留。","admin":false,"kind":"registry","fields":ads}),
        json!({"id":"diagnostics","label":"关闭可选诊断与反馈","description":"降到当前版本允许的最低诊断级别，关闭反馈提醒；不承诺停止所有必需诊断。","admin":true,"kind":"registry","fields":[
            field(r"HKLM\SOFTWARE\Policies\Microsoft\Windows\DataCollection","AllowTelemetry",1,3),
            field(r"HKLM\SOFTWARE\Policies\Microsoft\Windows\DataCollection","DoNotShowFeedbackNotifications",1,0),
            field(r"HKCU\Software\Microsoft\Siuf\Rules","NumberOfSIUFInPeriod",0,1)]}),
        json!({"id":"notifications","label":"关闭应用通知","description":"关闭应用横幅通知；可能错过消息提醒，安全防护不变。","admin":false,"kind":"registry","fields":[field(r"HKCU\Software\Microsoft\Windows\CurrentVersion\PushNotifications","ToastEnabled",0,1)]}),
        json!({"id":"game-mode","label":"启用游戏模式","description":"启用 Windows 游戏模式，不修改安全或计时器设置。","admin":false,"kind":"registry","fields":[field(r"HKCU\Software\Microsoft\GameBar","AllowAutoGameMode",1,0),field(r"HKCU\Software\Microsoft\GameBar","AutoGameModeEnabled",1,0)]}),
        json!({"id":"recording","label":"关闭游戏录制","description":"关闭游戏录制与后台捕获；独立截图组件保留。","admin":true,"kind":"registry","fields":[field(r"HKCU\Software\Microsoft\Windows\CurrentVersion\GameDVR","HistoricalCaptureEnabled",0,1),field(r"HKCU\Software\Microsoft\Windows\CurrentVersion\GameDVR","AppCaptureEnabled",0,1),field(r"HKCU\System\GameConfigStore","GameDVR_Enabled",0,1),field(r"HKLM\SOFTWARE\Policies\Microsoft\Windows\GameDVR","AllowGameDVR",0,1)]}),
        json!({"id":"delivery-upload","label":"关闭更新共享上传","description":"关闭传递优化的电脑间共享，保留官方 HTTP 更新下载。","admin":true,"kind":"registry","fields":[field(r"HKLM\SOFTWARE\Policies\Microsoft\Windows\DeliveryOptimization","DODownloadMode",0,1)]}),
        json!({"id":"storage-cleanup","label":"启用安全自动清理","description":"Windows 自动清理未使用的临时文件；不自动清空下载、回收站或云文件。","admin":true,"kind":"registry","fields":[field(storage,"AllowStorageSenseGlobal",1,0),field(storage,"AllowStorageSenseTemporaryFilesCleanup",1,0),field(storage,"ConfigStorageSenseGlobalCadence",7,0),field(storage,"ConfigStorageSenseDownloadsCleanupThreshold",0,0),field(storage,"ConfigStorageSenseRecycleBinCleanupThreshold",0,30),field(storage,"ConfigStorageSenseCloudContentDehydrationThreshold",0,0)]}),
        json!({"id":"disk-schedule","label":"暂停自动磁盘优化","description":"停用定期磁盘维护；需要整理或 TRIM 时须手动运行磁盘优化。","admin":true,"kind":"disk","fields":[]}),
        json!({"id":"power","label":"高性能","description":"使用本机已有的最高性能计划；增加耗电和发热，关闭恢复原计划。","admin":false,"kind":"power","fields":[]}),
        json!({"id":"local-search","label":"搜索仅显示本地内容","description":"关闭 Windows 搜索的网页建议。","admin":false,"kind":"registry","fields":[field(r"HKCU\Software\Policies\Microsoft\Windows\Explorer","DisableSearchBoxSuggestions",1,0)]}),
        json!({"id":"autoplay","label":"关闭移动设备自动播放","description":"插入移动设备时不再自动播放。","admin":false,"kind":"registry","fields":[field(r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\AutoplayHandlers","DisableAutoplay",1,0)]}),
        json!({"id":"activity-upload","label":"关闭活动历史上传","description":"不发布或上传应用活动记录；保留本地剪贴板历史。","admin":true,"kind":"registry","fields":[field(r"HKLM\SOFTWARE\Policies\Microsoft\Windows\System","PublishUserActivities",0,1),field(r"HKLM\SOFTWARE\Policies\Microsoft\Windows\System","UploadUserActivities",0,1)]}),
        json!({"id":"typing-privacy","label":"关闭输入个性化收集","description":"不收集键入、手写及通讯录训练数据；个性化输入建议可能减少。","admin":false,"kind":"registry","fields":[field(r"HKCU\Software\Microsoft\Input\TIPC","Enabled",0,1),field(r"HKCU\Software\Microsoft\InputPersonalization","RestrictImplicitInkCollection",1,0),field(r"HKCU\Software\Microsoft\InputPersonalization","RestrictImplicitTextCollection",1,0),field(r"HKCU\Software\Microsoft\InputPersonalization\TrainedDataStore","HarvestContacts",0,1)]}),
        json!({"id":"online-speech","label":"关闭在线语音识别","description":"停止在线语音识别授权；需要联网的语音输入将不可用。","admin":false,"kind":"registry","fields":[field(r"HKCU\Software\Microsoft\Speech_OneCore\Settings\OnlineSpeechPrivacy","HasAccepted",0,1)]}),
        json!({"id":"edge-background","label":"关闭 Edge 后台驻留","description":"关闭启动预加载和关窗后的后台运行；下次打开浏览器可能稍慢。","admin":true,"kind":"registry","fields":[field(r"HKLM\SOFTWARE\Policies\Microsoft\Edge","StartupBoostEnabled",0,1),field(r"HKLM\SOFTWARE\Policies\Microsoft\Edge","BackgroundModeEnabled",0,1)]}),
        json!({"id":"edge-promotions","label":"减少 Edge 推广","description":"关闭浏览器推荐与个性化报告；不卸载 Edge 或应用依赖的 WebView。","admin":true,"kind":"registry","fields":[field(r"HKLM\SOFTWARE\Policies\Microsoft\Edge","ShowRecommendationsEnabled",0,1),field(r"HKLM\SOFTWARE\Policies\Microsoft\Edge","PersonalizationReportingEnabled",0,1)]}),
    ]
}
fn definition(id: &str) -> Result<Value, String> {
    catalog()
        .into_iter()
        .find(|v| v["id"] == id)
        .ok_or("未知优化项目".into())
}
pub fn valid_operation(operation: &str) -> bool {
    if let Some(indices) = operation
        .strip_prefix("bundle-")
        .and_then(|s| s.strip_suffix("-on"))
    {
        let ids: Vec<_> = indices.split('_').collect();
        let mut seen = std::collections::HashSet::new();
        return !ids.is_empty()
            && ids.len() <= catalog().len()
            && ids.iter().all(|i| {
                i.parse::<usize>()
                    .ok()
                    .is_some_and(|n| n < catalog().len() && seen.insert(n))
            });
    }
    operation
        .strip_prefix("preset-")
        .and_then(|op| op.rsplit_once('-'))
        .is_some_and(|(id, mode)| definition(id).is_ok() && matches!(mode, "on" | "off"))
}
pub fn helper_script(operation: &str) -> Result<String, String> {
    if !valid_operation(operation) {
        return Err("无效优化操作".into());
    }
    if let Some(indices) = operation
        .strip_prefix("bundle-")
        .and_then(|s| s.strip_suffix("-on"))
    {
        let definitions = catalog();
        let ids = indices
            .split('_')
            .map(|i| {
                format!(
                    "'{}'",
                    definitions[i.parse::<usize>().unwrap()]["id"]
                        .as_str()
                        .unwrap()
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        return Ok(format!("$catalog='{}'|ConvertFrom-Json\n$results=@(foreach($selectedId in @({ids})){{$presetId=$selectedId;$mode='on';$raw=& {{\n{SCRIPT}\n}};$r=$raw|ConvertFrom-Json;[pscustomobject]@{{id=$selectedId;success=$r.success;message=$r.message}}}})\n@{{success=$true;message='优化设置处理完成';logs=(ConvertTo-Json -InputObject $results -Depth 6 -Compress);restart=$false}}|ConvertTo-Json -Compress",crate::network::runtime::quote(&serde_json::to_string(&definitions).map_err(|e|e.to_string())?)));
    }
    let (id, mode) = operation
        .strip_prefix("preset-")
        .unwrap()
        .rsplit_once('-')
        .unwrap();
    Ok(format!(
        "$catalog='{}'|ConvertFrom-Json;$presetId='{id}';$mode='{mode}'\n{SCRIPT}",
        crate::network::runtime::quote(
            &serde_json::to_string(&catalog()).map_err(|e| e.to_string())?
        )
    ))
}
fn root() -> Result<std::path::PathBuf, String> {
    let root = crate::local_app_data_dir()
        .ok_or("无法定位优化记录")?
        .join("WinToolbox/SystemRepair");
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    if !crate::cleaning::no_reparse_ancestors(&root) {
        return Err("优化记录目录包含链接".into());
    }
    Ok(root)
}
#[tauri::command]
pub async fn get_optimization_presets() -> Result<Vec<Preset>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let root = root()?;
        let script = format!(
            "$jobRoot='{}';$catalog='{}'|ConvertFrom-Json;$mode='read'\n{SCRIPT}",
            crate::network::runtime::quote(&root.to_string_lossy()),
            crate::network::runtime::quote(
                &serde_json::to_string(&catalog()).map_err(|e| e.to_string())?
            )
        );
        serde_json::from_str(&crate::network::runtime::run(&root, &script, 60)?)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn set_optimization_preset(
    id: String,
    enabled: bool,
    confirmed: bool,
) -> Result<String, String> {
    if !confirmed {
        return Err("请确认优化项目".into());
    }
    let definition = definition(&id)?;
    let _lease = Lease::enter(&[id.clone()])?;
    let operation = format!("preset-{id}-{}", if enabled { "on" } else { "off" });
    let result = if definition["admin"] == true {
        crate::system_repair::run_repair(operation).await?
    } else {
        tauri::async_runtime::spawn_blocking(move || {
            let root = root()?;
            let script = format!(
                "$jobRoot='{}'\n{}",
                crate::network::runtime::quote(&root.to_string_lossy()),
                helper_script(&operation)?
            );
            serde_json::from_str::<crate::system_repair::RepairResult>(
                &crate::network::runtime::run(&root, &script, 120)?,
            )
            .map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())??
    };
    if result.success {
        if matches!(id.as_str(), "local-search" | "privacy-ads") {
            crate::management::repair_taskbar()
                .await
                .map_err(|e| format!("配置已保存，资源管理器刷新失败：{e}"))?;
        }
        Ok(result.message)
    } else {
        Err(result.message)
    }
}
#[derive(Deserialize)]
pub struct PresetOutcome {
    pub id: String,
    pub success: bool,
    pub message: String,
}
pub async fn apply_bundle(ids: Vec<String>) -> Result<Vec<PresetOutcome>, String> {
    let definitions = catalog();
    let mut indices = Vec::new();
    let mut admin = false;
    for id in &ids {
        let index = definitions
            .iter()
            .position(|v| v["id"] == *id)
            .ok_or("未知优化项目")?;
        indices.push(index.to_string());
        admin |= definitions[index]["admin"] == true;
    }
    let operation = format!("bundle-{}-on", indices.join("_"));
    if !valid_operation(&operation) {
        return Err("优化选择无效".into());
    }
    let _lease = Lease::enter(&ids)?;
    let result = if admin {
        crate::system_repair::run_repair(operation).await?
    } else {
        tauri::async_runtime::spawn_blocking(move || {
            let root = root()?;
            let script = format!(
                "$jobRoot='{}'\n{}",
                crate::network::runtime::quote(&root.to_string_lossy()),
                helper_script(&operation)?
            );
            serde_json::from_str::<crate::system_repair::RepairResult>(
                &crate::network::runtime::run(&root, &script, 120)?,
            )
            .map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())??
    };
    if !result.success {
        return Err(result.message);
    }
    let mut rows: Vec<PresetOutcome> = serde_json::from_str(&result.logs)
        .map_err(|e| format!("优化结果读取失败，备份保留：{e}"))?;
    if rows
        .iter()
        .any(|r| r.success && matches!(r.id.as_str(), "local-search" | "privacy-ads"))
    {
        if let Err(error) = crate::management::repair_taskbar().await {
            for row in &mut rows {
                if row.success && matches!(row.id.as_str(), "local-search" | "privacy-ads") {
                    row.success = false;
                    row.message = format!("配置已保存，资源管理器刷新失败：{error}");
                }
            }
        }
    }
    Ok(rows)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unrelated_presets_remain_independent_and_conflicts_are_rejected() {
        let a = format!("a-{}", uuid::Uuid::new_v4());
        let b = format!("b-{}", uuid::Uuid::new_v4());
        let first = Lease::enter(&[a.clone()]).unwrap();
        let _second = Lease::enter(&[b]).unwrap();
        assert!(Lease::enter(&[a.clone()]).is_err());
        drop(first);
        assert!(Lease::enter(&[a]).is_ok());
    }
    #[test]
    fn renderer_cannot_supply_operations_or_paths() {
        for op in ["preset-power-on", "preset-privacy-ads-off"] {
            assert!(valid_operation(op));
        }
        assert!(valid_operation("bundle-0_1_2-on"));
        for op in [
            "bundle-0_0-on",
            "bundle-99-on",
            "bundle--on",
            "bundle-0-off",
        ] {
            assert!(!valid_operation(op));
        }
        for op in [
            "preset-nope-on",
            "preset-power-on;whoami",
            "preset-power-restore",
            "preset-../../on",
        ] {
            assert!(!valid_operation(op));
        }
    }
    #[test]
    fn storage_bundle_never_automatically_removes_personal_content() {
        let p = definition("storage-cleanup").unwrap();
        for name in [
            "ConfigStorageSenseDownloadsCleanupThreshold",
            "ConfigStorageSenseRecycleBinCleanupThreshold",
            "ConfigStorageSenseCloudContentDehydrationThreshold",
        ] {
            assert!(p["fields"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["name"] == name && v["on"] == 0));
        }
    }
    fn fixture(
        fail: bool,
        first_off: bool,
    ) -> (
        crate::system_repair::RepairResult,
        crate::system_repair::RepairResult,
        u32,
        bool,
    ) {
        use winreg::{enums::*, RegKey};
        let root = tempfile::tempdir().unwrap();
        let path = format!(r"Software\WinEaseTest\{}", uuid::Uuid::new_v4());
        let hive = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hive.create_subkey(&path).unwrap();
        key.set_value("first", &7u32).unwrap();
        let fields = vec![
            field(&format!("HKCU\\{path}"), "first", 0, 1),
            field(&format!("HKCU\\{path}"), "fail", 0, 1),
        ];
        let catalog = json!([{"id":"fixture","admin":false,"kind":"registry","fields":fields}]);
        let body = if fail {
            SCRIPT.replace("function Write-Field($f,$v) {","function Write-Field($f,$v) {if($f.name -eq 'fail' -and $v.exists -and $v.value -eq 0){throw 'fixture write failure'}")
        } else {
            SCRIPT.into()
        };
        let run = |mode: &str| -> crate::system_repair::RepairResult {
            let script=format!("$jobRoot='{}';$catalog='{}'|ConvertFrom-Json;$mode='{mode}';$presetId='fixture'\n{body}",crate::network::runtime::quote(&root.path().to_string_lossy()),crate::network::runtime::quote(&catalog.to_string()));
            serde_json::from_str(&crate::network::runtime::run(root.path(), &script, 45).unwrap())
                .unwrap()
        };
        let on = run(if first_off { "off" } else { "on" });
        let off = run("off");
        let value = key.get_value::<u32, _>("first").unwrap();
        let absent = key.get_value::<u32, _>("fail").is_err();
        drop(key);
        hive.delete_subkey_all(&path).unwrap();
        (on, off, value, absent)
    }
    #[test]
    fn grouped_registry_round_trip_preserves_values_and_absence() {
        let (on, off, value, absent) = fixture(false, false);
        assert!(on.success, "{}", on.message);
        assert!(off.success, "{}", off.message);
        assert_eq!(value, 7);
        assert!(absent);
    }
    #[test]
    fn grouped_failure_rolls_back_before_returning_error() {
        let (on, off, value, absent) = fixture(true, false);
        assert!(!on.success);
        assert!(on.message.contains("本次修改已回退"));
        assert!(off.success);
        assert_eq!(value, 7);
        assert!(absent);
    }
    #[test]
    fn first_off_without_history_uses_unoptimized_defaults() {
        let (first, again, value, absent) = fixture(false, true);
        assert!(first.success);
        assert!(again.success);
        assert_eq!(value, 1);
        assert!(!absent);
    }
    #[test]
    fn bundle_keeps_individual_results_and_restores_mock_power_and_disk() {
        use winreg::{enums::*, RegKey};
        let root = tempfile::tempdir().unwrap();
        let path = format!(r"Software\WinEaseTest\{}", uuid::Uuid::new_v4());
        let hive = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hive.create_subkey(&path).unwrap();
        key.set_value("first", &7u32).unwrap();
        let definitions = json!([
            {"id":"privacy-ads","admin":false,"kind":"registry","fields":[field(&format!("HKCU\\{path}"),"first",0,1)]},
            {"id":"game-mode","admin":false,"kind":"disk","fields":[]},
            {"id":"power","admin":false,"kind":"power","fields":[]}
        ]);
        let mut script = helper_script("bundle-0_3_8-on").unwrap();
        let end = script.find("|ConvertFrom-Json\n").unwrap() + "|ConvertFrom-Json\n".len();
        script.replace_range(
            ..end,
            &format!(
                "$catalog='{}'|ConvertFrom-Json\n",
                crate::network::runtime::quote(&definitions.to_string())
            ),
        );
        script = script.replace(
            r#"& "$env:windir\System32\powercfg.exe""#,
            "Invoke-FixturePower",
        );
        let prefix = format!(
            r#"$jobRoot='{}';$global:fixtureDisk=$true;$global:fixturePower='381b4222-f694-41f0-9685-ff5bb260df2e'
function Get-ScheduledTask {{param($TaskPath,$TaskName);[pscustomobject]@{{Settings=[pscustomobject]@{{Enabled=$global:fixtureDisk}}}}}}
function Enable-ScheduledTask {{param($TaskPath,$TaskName);$global:fixtureDisk=$true}}
function Disable-ScheduledTask {{param($TaskPath,$TaskName);$global:fixtureDisk=$false}}
function Invoke-FixturePower {{param($action,$guid);$global:LASTEXITCODE=0;if($action -eq '/list'){{'8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c'}}elseif($action -eq '/getactivescheme'){{$global:fixturePower}}elseif($action -eq '/setactive'){{$global:fixturePower=$guid}}else{{throw 'unexpected fixture command'}}}}
"#,
            crate::network::runtime::quote(&root.path().to_string_lossy())
        );
        let restore = SCRIPT.replace(
            r#"& "$env:windir\System32\powercfg.exe""#,
            "Invoke-FixturePower",
        );
        let fixture_catalog = crate::network::runtime::quote(&definitions.to_string());
        let script=format!("{prefix}$catalog='{fixture_catalog}'|ConvertFrom-Json;$raw=& {{\n{script}\n}};$batch=$raw|ConvertFrom-Json;$rows=$batch.logs|ConvertFrom-Json;if(@($rows).Count -ne 3 -or @($rows|Where-Object {{!$_.success}}).Count){{throw 'bundle failed'}};if($global:fixtureDisk -or $global:fixturePower -ne '8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c'){{throw 'mock actions not applied'}};foreach($presetId in @('privacy-ads','game-mode','power')){{$mode='off';$r=& {{\n{restore}\n}}|ConvertFrom-Json;if(!$r.success){{throw $r.message}}}};if(!$global:fixtureDisk -or $global:fixturePower -ne '381b4222-f694-41f0-9685-ff5bb260df2e'){{throw 'mock restore failed'}};'fixture complete'");
        let result = crate::network::runtime::run(root.path(), &script, 60);
        let value = key.get_value::<u32, _>("first").unwrap();
        drop(key);
        hive.delete_subkey_all(&path).unwrap();
        assert_eq!(result.unwrap().trim(), "fixture complete");
        assert_eq!(value, 7);
    }
    #[test]
    #[ignore = "read-only preset state; never changes user settings"]
    fn current_presets_are_readable() {
        let rows = tauri::async_runtime::block_on(get_optimization_presets()).unwrap();
        assert_eq!(rows.len(), 16);
        for row in rows {
            println!(
                "{}: {:?} {}",
                row.label,
                row.enabled,
                row.error.unwrap_or_default()
            );
        }
    }
}

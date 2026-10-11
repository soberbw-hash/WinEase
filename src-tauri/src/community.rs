//! Functional tuning: bounded read-only evidence and exact, reviewed actions.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::PathBuf, sync::Mutex};
static APP_LOCK: Mutex<()> = Mutex::new(());
static SAMPLE_LOCK: Mutex<()> = Mutex::new(());
static MAINTENANCE_LOCK: Mutex<()> = Mutex::new(());
fn root() -> Result<PathBuf, String> {
    let root = crate::local_app_data_dir()
        .ok_or("无法定位诊断记录")?
        .join("WinToolbox/Community");
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    if !crate::cleaning::no_reparse_ancestors(&root) {
        return Err("任务目录包含链接，已停止操作".into());
    }
    Ok(root)
}
fn script(body: &str, seconds: u64) -> Result<String, String> {
    crate::network::runtime::run(&root()?, body, seconds)
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceProcess {
    pub pid: u32,
    pub stamp: String,
    pub name: String,
    pub path: String,
    pub cpu_percent: f64,
    pub memory_bytes: u64,
    pub samples: usize,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Boot {
    pub duration_ms: u64,
    pub time: String,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupEvidence {
    pub name: Option<String>,
    pub path: Option<String>,
    pub duration_ms: Option<u64>,
    pub delay_ms: Option<u64>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceReport {
    pub seconds: f64,
    pub processes: Vec<ResourceProcess>,
    pub boot: Option<Boot>,
    pub startup: Vec<StartupEvidence>,
    pub boot_error: Option<String>,
    #[serde(default)]
    pub groups: Vec<ResourceGroup>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceGroup {
    pub name: String,
    pub path: String,
    pub cpu_percent: f64,
    pub memory_bytes: u64,
    pub process_count: usize,
    pub attention: bool,
}
pub fn resource_groups(report: &ResourceReport) -> Vec<ResourceGroup> {
    let mut groups: BTreeMap<String, ResourceGroup> = BTreeMap::new();
    for p in &report.processes {
        if p.samples < 4 || !p.cpu_percent.is_finite() || !(0.0..=100.0).contains(&p.cpu_percent) {
            continue;
        }
        let row = groups
            .entry(p.path.to_lowercase())
            .or_insert_with(|| ResourceGroup {
                name: p.name.clone(),
                path: p.path.clone(),
                cpu_percent: 0.0,
                memory_bytes: 0,
                process_count: 0,
                attention: false,
            });
        row.cpu_percent += p.cpu_percent;
        row.memory_bytes = row.memory_bytes.saturating_add(p.memory_bytes);
        row.process_count += 1;
    }
    let mut rows: Vec<_> = groups.into_values().collect();
    for r in &mut rows {
        r.cpu_percent = r.cpu_percent.min(100.0);
        r.attention = r.cpu_percent >= 15.0 || r.memory_bytes >= 2 * 1024 * 1024 * 1024;
    }
    rows.sort_by(|a, b| {
        b.cpu_percent
            .total_cmp(&a.cpu_percent)
            .then_with(|| b.memory_bytes.cmp(&a.memory_bytes))
    });
    rows
}
#[tauri::command]
pub async fn analyze_resources() -> Result<ResourceReport, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let _guard = SAMPLE_LOCK.try_lock().map_err(|_| "资源分析正在进行")?;
        let mut report: ResourceReport = serde_json::from_str(&script(
            include_str!("../resources/resource-analysis.ps1"),
            45,
        )?)
        .map_err(|e| e.to_string())?;
        report.groups = resource_groups(&report);
        Ok(report)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[derive(Clone, Deserialize, Serialize)]
pub struct MaintenanceCheck {
    pub id: String,
    pub title: String,
    pub status: String,
    pub detail: String,
    pub repair: Option<String>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaintenanceReport {
    pub checks: Vec<MaintenanceCheck>,
    pub pending_reboot: bool,
    #[serde(default)]
    pub can_restore_services: bool,
}
#[tauri::command]
pub async fn check_maintenance() -> Result<MaintenanceReport, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let mut report: MaintenanceReport = serde_json::from_str(&script(
            include_str!("../resources/maintenance-check.ps1"),
            60,
        )?)
        .map_err(|e| e.to_string())?;
        let backup = crate::local_app_data_dir()
            .ok_or("无法定位恢复记录")?
            .join("WinToolbox/SystemRepair/update-service-backup.json");
        report.can_restore_services =
            backup.is_file() && crate::cleaning::no_reparse_ancestors(&backup);
        Ok(report)
    })
    .await
    .map_err(|e| e.to_string())?
}
fn register_script(name: &str) -> String {
    format!(
        r#"$ErrorActionPreference='Stop'
$p=Get-AppxPackage -Name '{name}' | Select-Object -First 1
if(!$p){{throw '应用未安装，请通过 Microsoft Store 重新安装'}}
Add-AppxPackage -RegisterByFamilyName -MainPackage $p.PackageFamilyName -ErrorAction Stop
$again=Get-AppxPackage -Name '{name}' | Select-Object -First 1
if(!$again -or [string]$again.Status -ne 'Ok'){{throw '重新注册后包状态仍异常'}}
'已重新注册并核对包状态；应用数据保留。'"#
    )
}
#[tauri::command]
pub async fn repair_maintenance(action: String, confirmed: bool) -> Result<String, String> {
    if !confirmed {
        return Err("请先确认修复范围".into());
    }
    if action == "update-services" {
        return crate::system_repair::repair_update_services()
            .await
            .map(|r| {
                if r.success {
                    Ok(r.message)
                } else {
                    Err(r.message)
                }
            })?;
    }
    tauri::async_runtime::spawn_blocking(move|| {
        let _guard=MAINTENANCE_LOCK.try_lock().map_err(|_|"应用安装维护正在进行")?;
        let body=match action.as_str() {
            "winget-register"=>register_script("Microsoft.DesktopAppInstaller"),
            "store-register"=>register_script("Microsoft.WindowsStore"),
            "store-cache"=>r#"$ErrorActionPreference='Stop';$p=Start-Process -FilePath "$env:windir\System32\wsreset.exe" -WindowStyle Hidden -PassThru -Wait;if($p.ExitCode -ne 0){throw ('商店缓存重置退出码 '+$p.ExitCode)};'缓存重置命令已完成；请复查原来的商店下载或登录问题。'"#.into(),
            "winget-source"=>r#"$ErrorActionPreference='Stop';$text=& winget.exe source update --name winget --disable-interactivity 2>&1;if($LASTEXITCODE -ne 0){throw ($text -join "`n")};$text -join "`n""#.into(),
            "update-services-restore"=>return Err("请在更新服务的恢复入口操作".into()),
            _=>return Err("未知维护操作".into()),
        };
        let message=script(&body,120)?;
        if action=="winget-register" {
            script("$ErrorActionPreference='Stop';$v=& winget.exe --version 2>&1;if($LASTEXITCODE -ne 0){throw '包已重新注册，但 WinGet 命令仍不可用，请检查执行别名'};$v -join ' '",20)?;
        }
        Ok(message)
    }).await.map_err(|e|e.to_string())?
}

// Optional applications only. Frameworks, system shell, runtimes, Store, Xbox services,
// WebView and security packages are never removed based on a fuzzy name or publisher.
const OPTIONAL_APPS: &[(&str, &str, &str)] = &[
    (
        "Microsoft.BingNews",
        "Microsoft 资讯",
        "新闻阅读；不用时可移除",
    ),
    (
        "Microsoft.BingWeather",
        "Microsoft 天气",
        "天气应用；不用时可移除",
    ),
    (
        "Microsoft.MicrosoftSolitaireCollection",
        "Microsoft 纸牌",
        "休闲游戏；不用时可移除",
    ),
    ("Microsoft.Getstarted", "Windows 提示", "系统使用提示"),
    (
        "Microsoft.WindowsFeedbackHub",
        "反馈中心",
        "提交 Windows 反馈",
    ),
    (
        "Microsoft.GetHelp",
        "获取帮助",
        "微软支持与疑难解答；移除后会影响这些入口",
    ),
    (
        "Microsoft.MicrosoftOfficeHub",
        "Microsoft 365 应用入口",
        "Office 应用入口；不卸载桌面版 Office",
    ),
    (
        "Microsoft.ZuneMusic",
        "媒体播放器",
        "播放音频；移除后需选择其他播放器",
    ),
    ("Microsoft.ZuneVideo", "电影和电视", "视频播放与购买内容"),
    (
        "Clipchamp.Clipchamp",
        "Clipchamp",
        "视频剪辑；移除可能删除本地草稿数据",
    ),
    ("Microsoft.WindowsMaps", "Windows 地图", "地图应用"),
    ("Microsoft.People", "人脉", "联系人应用"),
];
fn optional_app(name: &str) -> Option<(&'static str, &'static str)> {
    OPTIONAL_APPS
        .iter()
        .find(|r| r.0 == name)
        .map(|r| (r.1, r.2))
}
fn classify_apps(rows: &mut [AppPackage]) {
    let depended: std::collections::HashSet<_> = rows
        .iter()
        .flat_map(|r| r.dependencies.iter().cloned())
        .collect();
    for row in rows {
        if let Some((label, purpose)) = optional_app(&row.name) {
            row.label = label.into();
            row.purpose = purpose.into();
            row.removable = !row.framework && !row.non_removable && !depended.contains(&row.name);
        } else {
            row.label = row.name.clone();
            row.purpose = "系统组件、运行环境或未审查应用；保留，通过原应用管理。".into();
            row.removable = false;
        }
    }
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPackage {
    pub name: String,
    pub full_name: String,
    pub family: String,
    pub version: String,
    pub location: String,
    pub framework: bool,
    pub non_removable: bool,
    pub dependencies: Vec<String>,
    pub size_bytes: Option<u64>,
    pub size_limited: bool,
    #[serde(default)]
    pub icon_target: Option<String>,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub purpose: String,
    #[serde(default)]
    pub removable: bool,
    #[serde(default)]
    pub removed: bool,
}
fn record_path(name: &str) -> Result<PathBuf, String> {
    if optional_app(name).is_none() {
        return Err("此应用受保护或不在可选应用清单".into());
    }
    Ok(root()?.join(format!("app-{name}.json")))
}
fn read_apps() -> Result<Vec<AppPackage>, String> {
    let body = r#"$ErrorActionPreference='Stop'
$rows=@(Get-AppxPackage -PackageTypeFilter Main | ForEach-Object {
 $p=$_;$bytes=$null;$limited=$false;$iconTarget=$null
 if($reviewed -contains $p.Name){try {$manifest=Get-AppxPackageManifest -Package $p;$executable=[string]@($manifest.Package.Applications.Application)[0].Executable;if($executable -and !$executable.Contains("..")){$candidate=Join-Path $p.InstallLocation $executable;if(Test-Path -LiteralPath $candidate -PathType Leaf){$iconTarget=$candidate}}}catch {}}
 if($reviewed -contains $p.Name -and $p.InstallLocation -and !$p.IsFramework -and !$p.NonRemovable) {
  try {
   $bytes=[uint64]0;$watch=[Diagnostics.Stopwatch]::StartNew();$count=0
   foreach($f in [IO.Directory]::EnumerateFiles($p.InstallLocation,'*',[IO.SearchOption]::AllDirectories)) {
    $count++;if($count -gt 20000 -or $watch.Elapsed.TotalSeconds -gt 1){$limited=$true;break}
    $bytes+=(Get-Item -LiteralPath $f -ErrorAction Stop).Length
   }
  } catch {$limited=$true;$bytes=$null}
 }
 [pscustomobject]@{name=$p.Name;fullName=$p.PackageFullName;family=$p.PackageFamilyName;version=[string]$p.Version;location=[string]$p.InstallLocation;framework=[bool]$p.IsFramework;nonRemovable=[bool]$p.NonRemovable;iconTarget=$iconTarget;dependencies=@($p.Dependencies | ForEach-Object {[string]$_.Name});sizeBytes=$bytes;sizeLimited=$limited}
})
ConvertTo-Json -InputObject $rows -Depth 5 -Compress"#;
    let reviewed = serde_json::to_string(&OPTIONAL_APPS.iter().map(|r| r.0).collect::<Vec<_>>())
        .map_err(|e| e.to_string())?;
    let body = format!(
        "$reviewed='{}'|ConvertFrom-Json\n{body}",
        crate::network::runtime::quote(&reviewed)
    );
    let mut rows: Vec<AppPackage> =
        serde_json::from_str(&script(&body, 90)?).map_err(|e| e.to_string())?;
    classify_apps(&mut rows);
    rows.sort_by_key(|r| (!r.removable, r.label.clone()));
    Ok(rows)
}
#[tauri::command]
pub async fn list_preinstalled_apps() -> Result<Vec<AppPackage>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let _guard = APP_LOCK.try_lock().map_err(|_| "预装应用任务正在进行")?;
        let mut rows = read_apps()?;
        for entry in fs::read_dir(root()?).map_err(|e| e.to_string())?.flatten() {
            let path = entry.path();
            if !path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("app-"))
                || path.extension().and_then(|e| e.to_str()) != Some("json")
            {
                continue;
            }
            if let Ok(mut row) =
                serde_json::from_slice::<AppPackage>(&fs::read(path).map_err(|e| e.to_string())?)
            {
                if optional_app(&row.name).is_some() && !rows.iter().any(|p| p.name == row.name) {
                    row.removed = true;
                    row.removable = false;
                    rows.push(row);
                }
            }
        }
        Ok(rows)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn manage_preinstalled_app(
    name: String,
    full_name: String,
    restore: bool,
    confirmed: bool,
) -> Result<String, String> {
    if !confirmed {
        return Err("请先确认应用与数据影响".into());
    }
    tauri::async_runtime::spawn_blocking(move|| {
        let _guard=APP_LOCK.try_lock().map_err(|_|"预装应用任务正在进行")?;
        let path=record_path(&name)?;
        if restore {
            let row:AppPackage=serde_json::from_slice(&fs::read(&path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
            if row.name!=name || !row.family.starts_with(&(name.clone()+"_")) || !row.family.chars().all(|c|c.is_ascii_alphanumeric()||c=='.'||c=='_'){return Err("恢复记录不匹配".into());}
            let body=format!("$ErrorActionPreference='Stop';Add-AppxPackage -RegisterByFamilyName -MainPackage '{}' -ErrorAction Stop;if(!(Get-AppxPackage -Name '{}')){{throw '应用恢复后未注册'}};'应用已重新注册；原应用数据不保证恢复。'",row.family,crate::network::runtime::quote(&name));
            match script(&body,120) {
                Ok(message)=>{fs::remove_file(path).map_err(|e|e.to_string())?;Ok(message)},
                Err(error)=>Err(format!("本机已无可注册安装包或恢复失败：{error}。请通过 Microsoft Store 重新安装，恢复记录已保留。")),
            }
        } else {
            let rows=read_apps()?;
            let row=rows.iter().find(|r|r.name==name && r.full_name==full_name && r.removable).ok_or("包已变化、被依赖或不可移除，请重新扫描")?;
            let bytes=serde_json::to_vec_pretty(row).map_err(|e|e.to_string())?;
            use std::io::Write;
            if path.exists() {
                let existing:AppPackage=serde_json::from_slice(&fs::read(&path).map_err(|e|e.to_string())?).map_err(|_|"恢复记录损坏，请保留记录并检查")?;
                if existing.name!=row.name || existing.family!=row.family {return Err("恢复记录不匹配，已停止".into());}
                // A previous failed removal retains a journal. Reuse only the exact same package.
                if existing.full_name!=row.full_name{return Err("原恢复记录对应另一版本，请先处理该记录".into());}
            } else {
            let mut file=fs::OpenOptions::new().write(true).create_new(true).open(&path).map_err(|e|format!("无法保存恢复记录：{e}"))?;
            if let Err(error)=file.write_all(&bytes).and_then(|_|file.sync_all()) {
                drop(file);let _=fs::remove_file(&path);return Err(format!("恢复记录未完成：{error}"));
            }
            }
            let body=format!("$ErrorActionPreference='Stop';Remove-AppxPackage -Package '{}' -ErrorAction Stop;if(Get-AppxPackage -Name '{}'){{throw '当前用户仍注册此应用，请刷新'}};'已从当前用户移除；其他账户与系统预配保留。'",crate::network::runtime::quote(&row.full_name),crate::network::runtime::quote(&name));
            script(&body,120)
        }
    }).await.map_err(|e|e.to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn critical_packages_and_fuzzy_names_are_not_optional() {
        for name in [
            "Microsoft.WindowsStore",
            "Microsoft.DesktopAppInstaller",
            "Microsoft.VCLibs",
            "Microsoft.XboxGamingOverlay",
            "Microsoft.BingNews.Bad",
            "Microsoft.Windows.ShellExperienceHost",
        ] {
            assert!(optional_app(name).is_none());
        }
    }
    #[test]
    fn sustained_resource_evidence_groups_only_valid_samples() {
        let mut report = ResourceReport {
            seconds: 6.0,
            processes: vec![],
            boot: None,
            startup: vec![],
            boot_error: None,
            groups: vec![],
        };
        for (path, cpu, samples) in [
            (r"C:\Apps\Chrome.exe", 9.0, 6),
            (r"c:\apps\chrome.exe", 9.0, 6),
            (r"C:\Other.exe", 99.0, 1),
            (r"C:\Bad.exe", f64::NAN, 6),
        ] {
            report.processes.push(ResourceProcess {
                pid: 5,
                stamp: "123".into(),
                name: "chrome".into(),
                path: path.into(),
                cpu_percent: cpu,
                memory_bytes: 1024,
                samples,
            });
        }
        let groups = resource_groups(&report);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].process_count, 2);
        assert!(groups[0].attention);
        assert_eq!(groups[0].memory_bytes, 2048);
    }
    #[test]
    fn app_dependencies_and_protection_override_optional_catalog() {
        let data = r#"[{"name":"Microsoft.BingWeather","fullName":"weather","family":"weather","version":"1","location":"","framework":false,"nonRemovable":false,"dependencies":[],"sizeBytes":null,"sizeLimited":true},{"name":"Other.App","fullName":"other","family":"other","version":"1","location":"","framework":false,"nonRemovable":false,"dependencies":["Microsoft.BingWeather"],"sizeBytes":null,"sizeLimited":false}]"#;
        let mut rows: Vec<AppPackage> = serde_json::from_str(data).unwrap();
        classify_apps(&mut rows);
        assert!(!rows[0].removable);
        assert!(!rows[1].removable);
        rows[1].dependencies.clear();
        classify_apps(&mut rows);
        assert!(rows[0].removable);
        rows[0].non_removable = true;
        classify_apps(&mut rows);
        assert!(!rows[0].removable);
    }
    #[test]
    #[ignore = "read-only resource, maintenance and current-user package lookup; never repairs or uninstalls"]
    fn current_community_checks_are_readable() {
        tauri::async_runtime::block_on(async {
            let report = analyze_resources().await.unwrap();
            assert!(report.seconds >= 6.0);
            println!("resource groups {}", resource_groups(&report).len());
            let report = check_maintenance().await.unwrap();
            assert!(!report.checks.is_empty());
            println!("maintenance checks {}", report.checks.len());
            let rows = list_preinstalled_apps().await.unwrap();
            println!(
                "packages {}, optional {}",
                rows.len(),
                rows.iter().filter(|r| r.removable).count()
            );
        });
    }
}

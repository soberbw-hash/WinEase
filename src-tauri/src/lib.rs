mod application_icons;
mod cleaning;
mod component_launcher;
mod component_updates;
mod file_management;
mod management;
mod network;
mod system_repair;
pub fn network_helper_entry() -> bool {
    network::helper_entry() || system_repair::helper_entry()
}
mod popups;
mod recycle;
mod windows_settings;
use encoding_rs::GBK;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tauri::Manager;
use walkdir::WalkDir;

const CREATE_NO_WINDOW: u32 = 0x08000000;

const HIGH_PERFORMANCE_GUID: &str = "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c";
const BALANCED_GUID: &str = "381b4222-f694-41f0-9685-ff5bb260df2e";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SystemSnapshot {
    host_name: String,
    os_name: String,
    os_version: String,
    os_build: String,
    cpu_name: String,
    cpu_load: u64,
    cpu_cores: u64,
    logical_cores: u64,
    memory_total_mb: u64,
    memory_used_mb: u64,
    memory_usage_percent: u64,
    gpu_name: Option<String>,
    gpu_memory_mb: Option<u64>,
    network_name: Option<String>,
    network_description: Option<String>,
    network_link_speed: Option<String>,
    collected_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolActionResult {
    action_id: String,
    title: String,
    success: bool,
    summary: String,
    details: String,
    duration_ms: u64,
    output_path: Option<String>,
    warnings: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ComponentManifest {
    id: String,
    name: String,
    description: String,
    category: String,
    kind: String,
    status: String,
    installed: bool,
    status_label: String,
    summary: String,
    version: Option<String>,
    source_label: Option<String>,
    source_url: Option<String>,
    license_name: Option<String>,
    license_url: Option<String>,
    install_size: Option<String>,
    winget_id: Option<String>,
    homepage: Option<String>,
    launch_path: Option<String>,
    launch_arguments: Option<Vec<String>>,
    install_dir: Option<String>,
    log_dir: Option<String>,
    supports_repair: bool,
    supports_uninstall: bool,
    supports_update: bool,
    recommended: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ThirdPartyNotice {
    id: String,
    name: String,
    version: String,
    source_label: String,
    source_url: String,
    license_name: String,
    license_url: Option<String>,
    notes: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct StorageHotspot {
    id: String,
    label: String,
    path: String,
    source: String,
    size_bytes: u64,
    item_count: u64,
}

#[derive(Debug)]
struct ProcessCapture {
    exit_code: Option<i32>,
    success: bool,
    stdout: String,
    stderr: String,
}

#[derive(Debug)]
struct ComponentDefinition {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    category: &'static str,
    kind: &'static str,
    version: Option<&'static str>,
    source_label: Option<&'static str>,
    source_url: Option<&'static str>,
    license_name: Option<&'static str>,
    license_url: Option<&'static str>,
    install_size: Option<&'static str>,
    winget_id: Option<&'static str>,
    homepage: Option<&'static str>,
    detect_paths: Vec<PathBuf>,
    launch_arguments: Vec<String>,
    install_dir_name: Option<&'static str>,
    recommended: bool,
    installed: bool,
    status: String,
    status_label: String,
    summary: String,
    supports_repair: bool,
    supports_uninstall: bool,
    supports_update: bool,
}

fn workspace_root() -> Option<PathBuf> {
    let mut cursor = std::env::current_dir().ok()?;

    loop {
        if cursor.join("package.json").exists() && cursor.join("src-tauri").exists() {
            return Some(cursor);
        }

        if !cursor.pop() {
            break;
        }
    }

    None
}

fn documents_dir() -> PathBuf {
    if let Some(user_profile) = std::env::var_os("USERPROFILE") {
        let candidate = PathBuf::from(user_profile).join("Documents");
        if candidate.exists() {
            return candidate;
        }
    }

    workspace_root()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn user_profile_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(PathBuf::from)
}

fn desktop_dir() -> Option<PathBuf> {
    user_profile_dir().map(|path| path.join("Desktop"))
}

fn downloads_dir() -> Option<PathBuf> {
    user_profile_dir().map(|path| path.join("Downloads"))
}

fn videos_dir() -> Option<PathBuf> {
    user_profile_dir().map(|path| path.join("Videos"))
}

fn pictures_dir() -> Option<PathBuf> {
    user_profile_dir().map(|path| path.join("Pictures"))
}

fn local_app_data_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
}

fn program_files_dir() -> Option<PathBuf> {
    std::env::var_os("ProgramFiles").map(PathBuf::from)
}

fn component_root() -> PathBuf {
    local_app_data_dir()
        .unwrap_or_else(documents_dir)
        .join("WinToolbox")
        .join("Components")
}

fn component_logs_root() -> PathBuf {
    local_app_data_dir()
        .unwrap_or_else(documents_dir)
        .join("WinToolbox")
        .join("logs")
        .join("components")
}

fn component_storage_dir(component: &ComponentDefinition) -> Option<PathBuf> {
    component
        .install_dir_name
        .map(|folder| component_root().join(folder))
}

fn component_log_dir(component: &ComponentDefinition) -> PathBuf {
    component_logs_root().join(component.id)
}

fn find_first_existing_path(paths: &[PathBuf]) -> Option<PathBuf> {
    paths
        .iter()
        .find(|path| path.is_file() && cleaning::no_reparse_ancestors(path))
        .cloned()
}

fn portable_package_executables(root: &Path, package_id: &str, names: &[String]) -> Vec<PathBuf> {
    if !cleaning::no_reparse_ancestors(root) {
        return Vec::new();
    }
    let prefix = format!("{}_", package_id.to_ascii_lowercase());
    let mut paths = Vec::new();
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.filter_map(Result::ok).take(1024) {
            let path = entry.path();
            if !entry
                .file_name()
                .to_string_lossy()
                .to_ascii_lowercase()
                .starts_with(&prefix)
                || !path.is_dir()
                || !cleaning::no_reparse_ancestors(&path)
            {
                continue;
            }
            for file in WalkDir::new(&path)
                .follow_links(false)
                .max_depth(4)
                .into_iter()
                .filter_entry(|e| cleaning::no_reparse_ancestors(e.path()))
                .take(1024)
                .filter_map(Result::ok)
            {
                if file.file_type().is_file()
                    && names.iter().any(|name| {
                        file.file_name()
                            .to_string_lossy()
                            .eq_ignore_ascii_case(name)
                    })
                {
                    paths.push(file.path().to_path_buf());
                }
            }
        }
    }
    paths.sort();
    paths
}

#[cfg(test)]
mod portable_component_tests {
    use super::*;
    #[test]
    fn package_lookup_requires_exact_id_and_known_executable() {
        let temp = tempfile::tempdir().unwrap();
        let good = temp
            .path()
            .join("BluePointLilac.ContextMenuManager_Source/nested");
        fs::create_dir_all(&good).unwrap();
        let executable = good.join("ContextMenuManager.exe");
        fs::write(&executable, b"fixture").unwrap();
        fs::write(good.join("Uninstall.exe"), b"fixture").unwrap();
        let wrong = temp
            .path()
            .join("BluePointLilac.ContextMenuManagerExtra_Source");
        fs::create_dir_all(&wrong).unwrap();
        fs::write(wrong.join("ContextMenuManager.exe"), b"fixture").unwrap();
        assert_eq!(
            portable_package_executables(
                temp.path(),
                "BluePointLilac.ContextMenuManager",
                &["ContextMenuManager.exe".into()]
            ),
            vec![executable]
        );
        assert!(portable_package_executables(
            temp.path(),
            "Other.Package",
            &["ContextMenuManager.exe".into()]
        )
        .is_empty());
    }
    #[test]
    #[ignore = "read-only installed ContextMenuManager lookup; never launches or repairs"]
    fn installed_context_menu_manager_portable_entry_is_found() {
        let root = local_app_data_dir()
            .unwrap()
            .join("Microsoft/WinGet/Packages");
        let paths = portable_package_executables(
            &root,
            "BluePointLilac.ContextMenuManager",
            &[
                "ContextMenuManager.exe".into(),
                "ContextMenuManager.NET.4.0.exe".into(),
            ],
        );
        assert!(find_first_existing_path(&paths).is_some());
    }

    #[test]
    #[ignore = "opens the installed component through the production button backend; may prompt for UAC"]
    fn installed_context_menu_manager_launches_through_button_backend() {
        let root = local_app_data_dir()
            .unwrap()
            .join("Microsoft/WinGet/Packages");
        let path = find_first_existing_path(&portable_package_executables(
            &root,
            "BluePointLilac.ContextMenuManager",
            &[
                "ContextMenuManager.exe".into(),
                "ContextMenuManager.NET.4.0.exe".into(),
            ],
        ))
        .unwrap();
        let direct = Command::new(&path)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
        assert_eq!(
            direct.unwrap_err().raw_os_error(),
            Some(740),
            "run this test without administrator permissions to exercise the fallback"
        );
        let result = launch_component_internal("context-menu-manager").unwrap();
        assert!(result.success, "{}", result.summary);
        println!("{}\n{}", result.summary, result.details);
    }
}

fn search_paths_for_executable(
    root: &Path,
    executable_name: &str,
    max_depth: usize,
) -> Vec<PathBuf> {
    if !root.exists() {
        return Vec::new();
    }

    WalkDir::new(root)
        .max_depth(max_depth)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(executable_name)
        })
        .map(|entry| entry.path().to_path_buf())
        .collect()
}

fn snipaste_detect_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(local) = local_app_data_dir() {
        paths.push(local.join("Programs").join("Snipaste").join("Snipaste.exe"));
        paths.extend(search_paths_for_executable(
            &local.join("Microsoft").join("WinGet").join("Packages"),
            "Snipaste.exe",
            4,
        ));
    }

    if let Some(program_files) = program_files_dir() {
        paths.push(program_files.join("Snipaste").join("Snipaste.exe"));
        paths.extend(search_paths_for_executable(
            &program_files,
            "Snipaste.exe",
            3,
        ));
    }

    paths
}

fn honeyview_detect_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(local) = local_app_data_dir() {
        paths.push(local.join("Honeyview").join("Honeyview.exe"));
        paths.extend(search_paths_for_executable(
            &local.join("Microsoft").join("WinGet").join("Packages"),
            "Honeyview.exe",
            4,
        ));
    }

    if let Some(program_files) = program_files_dir() {
        paths.push(program_files.join("Honeyview").join("Honeyview.exe"));
        paths.extend(search_paths_for_executable(
            &program_files,
            "Honeyview.exe",
            3,
        ));
    }

    paths
}

fn file_converter_detect_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(local) = local_app_data_dir() {
        paths.extend(search_paths_for_executable(
            &local.join("Microsoft").join("WinGet").join("Packages"),
            "FileConverter.exe",
            4,
        ));
    }

    if let Some(program_files) = program_files_dir() {
        paths.push(
            program_files
                .join("File Converter")
                .join("FileConverter.exe"),
        );
        paths.extend(search_paths_for_executable(
            &program_files,
            "FileConverter.exe",
            3,
        ));
    }

    paths
}

fn koodo_reader_detect_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(local) = local_app_data_dir() {
        paths.push(
            local
                .join("Programs")
                .join("Koodo Reader")
                .join("Koodo Reader.exe"),
        );
        paths.push(
            local
                .join("Programs")
                .join("koodo-reader")
                .join("Koodo Reader.exe"),
        );
        paths.extend(search_paths_for_executable(
            &local.join("Microsoft").join("WinGet").join("Packages"),
            "Koodo Reader.exe",
            4,
        ));
    }

    if let Some(program_files) = program_files_dir() {
        paths.push(program_files.join("Koodo Reader").join("Koodo Reader.exe"));
        paths.extend(search_paths_for_executable(
            &program_files,
            "Koodo Reader.exe",
            3,
        ));
    }

    paths
}

fn clash_verge_detect_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(local) = local_app_data_dir() {
        paths.push(
            local
                .join("Programs")
                .join("Clash Verge")
                .join("Clash Verge.exe"),
        );
        paths.push(
            local
                .join("Programs")
                .join("Clash Verge Rev")
                .join("Clash Verge.exe"),
        );
        paths.push(
            local
                .join("Programs")
                .join("Clash Verge Rev")
                .join("clash-verge.exe"),
        );
        paths.extend(search_paths_for_executable(
            &local.join("Microsoft").join("WinGet").join("Packages"),
            "clash-verge.exe",
            4,
        ));
        paths.extend(search_paths_for_executable(
            &local.join("Microsoft").join("WinGet").join("Packages"),
            "Clash Verge.exe",
            4,
        ));
    }

    if let Some(program_files) = program_files_dir() {
        paths.extend(search_paths_for_executable(
            &program_files,
            "clash-verge.exe",
            3,
        ));
        paths.extend(search_paths_for_executable(
            &program_files,
            "Clash Verge.exe",
            3,
        ));
    }

    paths
}

fn bcuninstaller_detect_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(local) = local_app_data_dir() {
        paths.push(
            local
                .join("Programs")
                .join("BCUninstaller")
                .join("BCUninstaller.exe"),
        );
        paths.extend(search_paths_for_executable(
            &local.join("Microsoft").join("WinGet").join("Packages"),
            "BCUninstaller.exe",
            4,
        ));
    }

    if let Some(program_files) = program_files_dir() {
        paths.push(
            program_files
                .join("BCUninstaller")
                .join("BCUninstaller.exe"),
        );
        paths.extend(search_paths_for_executable(
            &program_files,
            "BCUninstaller.exe",
            3,
        ));
    }

    paths
}

fn everything_detect_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(program_files) = program_files_dir() {
        paths.push(program_files.join("Everything").join("Everything.exe"));
    }

    paths
}

fn context_menu_manager_detect_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(local) = local_app_data_dir() {
        paths.push(
            local
                .join("Programs")
                .join("ContextMenuManager")
                .join("ContextMenuManager.exe"),
        );
    }

    if let Some(program_files) = program_files_dir() {
        paths.push(
            program_files
                .join("ContextMenuManager")
                .join("ContextMenuManager.exe"),
        );
    }

    paths
}

fn seven_zip_detect_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(program_files) = program_files_dir() {
        paths.push(program_files.join("7-Zip").join("7zFM.exe"));
    }

    paths
}

fn powertoys_detect_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(program_files) = program_files_dir() {
        paths.push(program_files.join("PowerToys").join("PowerToys.exe"));
    }

    if let Some(local) = local_app_data_dir() {
        paths.push(local.join("PowerToys").join("PowerToys.exe"));
    }

    paths
}

fn build_component_definition(
    id: &'static str,
    name: &'static str,
    description: &'static str,
    category: &'static str,
    version: Option<&'static str>,
    source_label: Option<&'static str>,
    source_url: Option<&'static str>,
    license_name: Option<&'static str>,
    license_url: Option<&'static str>,
    install_size: Option<&'static str>,
    winget_id: Option<&'static str>,
    homepage: Option<&'static str>,
    mut detect_paths: Vec<PathBuf>,
    launch_arguments: Vec<String>,
    install_dir_name: Option<&'static str>,
    recommended: bool,
    supports_repair: bool,
    supports_uninstall: bool,
    supports_update: bool,
) -> ComponentDefinition {
    if find_first_existing_path(&detect_paths).is_none() {
        if let Some(package_id) = winget_id {
            let mut names: Vec<String> = detect_paths
                .iter()
                .filter_map(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .collect();
            if id == "context-menu-manager" {
                names.push("ContextMenuManager.NET.4.0.exe".into());
            }
            names.sort();
            names.dedup();
            let roots = [
                local_app_data_dir().map(|p| p.join("Microsoft/WinGet/Packages")),
                program_files_dir().map(|p| p.join("WinGet/Packages")),
                std::env::var_os("ProgramFiles(x86)")
                    .map(|p| PathBuf::from(p).join("WinGet/Packages")),
            ];
            for root in roots.into_iter().flatten() {
                detect_paths.extend(portable_package_executables(&root, package_id, &names));
            }
        }
    }
    let launch_path = find_first_existing_path(&detect_paths);
    let winget_installed =
        launch_path.is_none() && winget_id.map(winget_package_installed).unwrap_or(false);
    let installed = launch_path.is_some() || winget_installed;
    let status = if installed && launch_path.is_none() && winget_id.is_some() {
        "repairable".to_string()
    } else if installed {
        "installed".to_string()
    } else {
        "not-installed".to_string()
    };

    ComponentDefinition {
        id,
        name,
        description,
        category,
        kind: if winget_id.is_some() {
            "winget"
        } else {
            "built-in"
        },
        version,
        source_label,
        source_url,
        license_name,
        license_url,
        install_size,
        winget_id,
        homepage,
        detect_paths,
        launch_arguments,
        install_dir_name,
        recommended,
        installed,
        status: status.clone(),
        status_label: if status == "repairable" {
            "入口未找到".to_string()
        } else if installed {
            "可用".to_string()
        } else {
            "未安装".to_string()
        },
        summary: if status == "repairable" {
            format!("{name} 已有安装记录，但尚未定位启动入口。")
        } else if installed {
            format!("{name} 已就绪，可以直接使用。")
        } else if winget_id.is_some() {
            format!("{name} 支持一键安装，装好就能直接用。")
        } else {
            format!("{name} 已内置在主程序中。")
        },
        supports_repair,
        supports_uninstall,
        supports_update,
    }
}

fn component_definitions_internal() -> Vec<ComponentDefinition> {
    let mut components = vec![
        ComponentDefinition {
            id: "capture-core",
            name: "基础截图",
            description: "Windows 区域截图。",
            category: "基础能力",
            kind: "built-in",
            version: None,
            source_label: None,
            source_url: None,
            license_name: None,
            license_url: None,
            install_size: None,
            winget_id: None,
            homepage: None,
            detect_paths: Vec::new(),
            launch_arguments: Vec::new(),
            install_dir_name: None,
            recommended: true,
            installed: true,
            status: "installed".to_string(),
            status_label: "可用".to_string(),
            summary: "区域截图开箱即用。".to_string(),
            supports_repair: false,
            supports_uninstall: false,
            supports_update: false,
        },
        build_component_definition(
            "image-viewer",
            "图片查看器",
            "轻量图片、动图查看器。",
            "效率组件",
            Some("5.53"),
            Some("winget · Honeyview"),
            Some("https://en.bandisoft.com/honeyview/"),
            Some("Freeware"),
            Some("https://en.bandisoft.com/honeyview/eula"),
            None,
            Some("Bandisoft.Honeyview"),
            Some("https://en.bandisoft.com/honeyview/"),
            honeyview_detect_paths(),
            Vec::new(),
            Some("ImageViewer"),
            true,
            false,
            true,
            false,
        ),
        build_component_definition(
            "file-converter",
            "File Converter",
            "右键转换和压缩文件。",
            "效率增强",
            Some("2.2"),
            Some("winget · File Converter"),
            Some("https://file-converter.io/"),
            Some("GPL-3.0"),
            Some("https://github.com/Tichau/FileConverter/blob/HEAD/LICENSE.md"),
            None,
            Some("AdrienAllard.FileConverter"),
            Some("https://file-converter.io/"),
            file_converter_detect_paths(),
            vec!["--settings".to_string()],
            Some("FileConverter"),
            false,
            true,
            true,
            true,
        ),
        build_component_definition(
            "koodo-reader",
            "Koodo Reader",
            "一键安装 Koodo Reader，快速导入和阅读电子书。",
            "阅读增强",
            Some("2.3.1"),
            Some("winget · Koodo Reader"),
            Some("https://www.koodoreader.com/zh"),
            Some("AGPL-3.0"),
            Some("https://github.com/koodo-reader/koodo-reader/blob/HEAD/LICENSE"),
            None,
            Some("AppByTroye.KoodoReader"),
            Some("https://www.koodoreader.com/zh"),
            koodo_reader_detect_paths(),
            Vec::new(),
            Some("KoodoReader"),
            false,
            true,
            true,
            true,
        ),
        build_component_definition(
            "capture-plus",
            "Snipaste 截图增强",
            "一键安装 Snipaste，开启后按 F1 截图，按 F3 贴图。",
            "截图增强",
            Some("2.11.3"),
            Some("winget · Snipaste"),
            Some("https://www.snipaste.com/"),
            Some("Freemium"),
            Some("https://docs.snipaste.com/pro"),
            None,
            Some("liule.Snipaste"),
            Some("https://www.snipaste.com/"),
            snipaste_detect_paths(),
            Vec::new(),
            Some("Snipaste"),
            true,
            true,
            true,
            false,
        ),
        build_component_definition(
            "everything-search",
            "Everything 搜索增强",
            "快速搜索文件与文件夹。",
            "效率增强",
            None,
            Some("winget · voidtools"),
            Some("https://www.voidtools.com/"),
            Some("专有免费软件"),
            None,
            None,
            Some("voidtools.Everything"),
            Some("https://www.voidtools.com/"),
            everything_detect_paths(),
            Vec::new(),
            Some("Everything"),
            false,
            true,
            true,
            false,
        ),
        build_component_definition(
            "context-menu-manager",
            "右键菜单管理",
            "一键安装右键菜单管理器，方便清理和整理系统右键项。",
            "效率增强",
            None,
            Some("winget · BluePointLilac"),
            Some("https://github.com/BluePointLilac/ContextMenuManager"),
            Some("开源软件"),
            None,
            None,
            Some("BluePointLilac.ContextMenuManager"),
            Some("https://github.com/BluePointLilac/ContextMenuManager"),
            context_menu_manager_detect_paths(),
            Vec::new(),
            Some("ContextMenuManager"),
            false,
            true,
            true,
            true,
        ),
        build_component_definition(
            "archive-tools",
            "压缩解压增强",
            "安装 7-Zip，补齐常见压缩格式支持。",
            "效率增强",
            None,
            Some("winget · 7-Zip"),
            Some("https://www.7-zip.org/"),
            Some("开源软件"),
            Some("https://www.7-zip.org/license.txt"),
            None,
            Some("7zip.7zip"),
            Some("https://www.7-zip.org/"),
            seven_zip_detect_paths(),
            Vec::new(),
            Some("ArchiveTools"),
            false,
            false,
            true,
            false,
        ),
        build_component_definition(
            "clash-verge-rev",
            "Clash Verge Rev",
            "Clash Verge Rev 网络代理工具。",
            "网络增强",
            None,
            Some("winget · Clash Verge Rev"),
            Some("https://github.com/clash-verge-rev/clash-verge-rev"),
            Some("GPL-3.0"),
            Some("https://github.com/clash-verge-rev/clash-verge-rev/blob/main/LICENSE"),
            None,
            Some("ClashVergeRev.ClashVergeRev"),
            Some("https://github.com/clash-verge-rev/clash-verge-rev"),
            clash_verge_detect_paths(),
            Vec::new(),
            Some("ClashVergeRev"),
            true,
            true,
            true,
            true,
        ),
        build_component_definition(
            "uninstall-plus",
            "BCUninstaller",
            "卸载应用并扫描残留文件、注册表项。",
            "系统增强",
            None,
            Some("winget · BCUninstaller"),
            Some("https://www.bcuninstaller.com/"),
            Some("Apache-2.0"),
            Some("https://github.com/BCUninstaller/Bulk-Crap-Uninstaller/blob/HEAD/Licence.txt"),
            None,
            Some("Klocman.BulkCrapUninstaller"),
            Some("https://www.bcuninstaller.com/"),
            bcuninstaller_detect_paths(),
            Vec::new(),
            Some("BCUninstaller"),
            true,
            false,
            true,
            false,
        ),
        build_component_definition(
            "powertoys-suite",
            "PowerToys",
            "Microsoft Windows 效率工具集。",
            "效率增强",
            None,
            Some("winget · Microsoft"),
            Some("https://github.com/microsoft/PowerToys"),
            Some("MIT"),
            Some("https://github.com/microsoft/PowerToys/blob/main/LICENSE"),
            None,
            Some("Microsoft.PowerToys"),
            Some("https://github.com/microsoft/PowerToys"),
            powertoys_detect_paths(),
            Vec::new(),
            Some("PowerToys"),
            false,
            false,
            true,
            false,
        ),
    ];

    components.sort_by(|left, right| {
        right
            .recommended
            .cmp(&left.recommended)
            .then(right.installed.cmp(&left.installed))
            .then(left.category.cmp(&right.category))
            .then(left.name.cmp(&right.name))
    });

    components
}

fn list_components_internal() -> Vec<ComponentManifest> {
    component_definitions_internal()
        .into_iter()
        .map(|item| {
            let launch_path = find_first_existing_path(&item.detect_paths)
                .map(|path| path.to_string_lossy().to_string());
            let install_dir =
                component_storage_dir(&item).map(|path| path.to_string_lossy().to_string());
            let log_dir = Some(component_log_dir(&item).to_string_lossy().to_string());

            ComponentManifest {
                id: item.id.to_string(),
                name: item.name.to_string(),
                description: item.description.to_string(),
                category: item.category.to_string(),
                kind: item.kind.to_string(),
                status: item.status,
                installed: item.installed,
                status_label: item.status_label,
                summary: item.summary,
                version: item.version.map(|value| value.to_string()),
                source_label: item.source_label.map(|value| value.to_string()),
                source_url: item.source_url.map(|value| value.to_string()),
                license_name: item.license_name.map(|value| value.to_string()),
                license_url: item.license_url.map(|value| value.to_string()),
                install_size: item.install_size.map(|value| value.to_string()),
                winget_id: item.winget_id.map(|value| value.to_string()),
                homepage: item.homepage.map(|value| value.to_string()),
                launch_path,
                launch_arguments: if item.launch_arguments.is_empty() {
                    None
                } else {
                    Some(item.launch_arguments)
                },
                install_dir,
                log_dir,
                supports_repair: item.supports_repair,
                supports_uninstall: item.supports_uninstall,
                supports_update: item.supports_update,
                recommended: item.recommended,
            }
        })
        .collect()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredComponentManifest {
    id: String,
    name: String,
    version: String,
    kind: String,
    install_dir: String,
    entry: String,
    display_name: String,
    category: String,
    supports_repair: bool,
    supports_uninstall: bool,
    supports_update: bool,
}

fn persist_component_state(component: &ComponentManifest) -> Result<Option<PathBuf>, String> {
    let install_dir = match component.install_dir.as_ref() {
        Some(path) => PathBuf::from(path),
        None => return Ok(None),
    };

    fs::create_dir_all(&install_dir)
        .map_err(|error| format!("无法创建组件数据目录 {}: {error}", install_dir.display()))?;

    let manifest_path = install_dir.join("component_manifest.json");
    let entry = component
        .launch_path
        .as_ref()
        .and_then(|path| Path::new(path).file_name())
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| format!("{}.exe", component.name.replace(' ', "")));

    let stored = StoredComponentManifest {
        id: component.id.clone(),
        name: component.name.clone(),
        version: component
            .version
            .clone()
            .unwrap_or_else(|| "当前版本".to_string()),
        kind: component.kind.clone(),
        install_dir: install_dir.to_string_lossy().to_string(),
        entry,
        display_name: component.name.clone(),
        category: component.category.clone(),
        supports_repair: component.supports_repair,
        supports_uninstall: component.supports_uninstall,
        supports_update: component.supports_update,
    };

    let raw = serde_json::to_string_pretty(&stored)
        .map_err(|error| format!("无法序列化组件信息：{error}"))?;
    fs::write(&manifest_path, raw).map_err(|error| format!("无法写入组件信息：{error}"))?;

    Ok(Some(manifest_path))
}

fn remove_component_state(component: &ComponentManifest) -> Result<(), String> {
    if let Some(path) = component.install_dir.as_ref() {
        let install_dir = PathBuf::from(path);
        if !install_dir.exists() {
            return Ok(());
        }
        if !install_dir.starts_with(component_root())
            || !cleaning::no_reparse_ancestors(&install_dir)
        {
            return Err("组件记录路径异常，已保留数据。".into());
        }
        let manifest = install_dir.join("component_manifest.json");
        if manifest.exists() {
            fs::remove_file(&manifest).map_err(|error| format!("无法删除组件记录：{error}"))?;
        }
    }

    Ok(())
}

fn write_component_log(
    component: &ComponentManifest,
    action: &str,
    content: &str,
) -> Result<PathBuf, String> {
    let log_dir = component
        .log_dir
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| component_logs_root().join(&component.id));

    fs::create_dir_all(&log_dir)
        .map_err(|error| format!("无法创建组件日志目录 {}: {error}", log_dir.display()))?;

    let log_path = log_dir.join(format!("{action}.log"));
    let entry = format!("[{}]\n{}\n\n", chrono_like_timestamp(), content.trim());

    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|error| format!("无法写入组件日志 {}: {error}", log_path.display()))?;
    file.write_all(entry.as_bytes())
        .map_err(|error| format!("无法写入组件日志 {}: {error}", log_path.display()))?;

    Ok(log_path)
}

fn chrono_like_timestamp() -> String {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => format!("unix-{}", duration.as_secs()),
        Err(_) => "unix-0".to_string(),
    }
}

fn run_component_install(component_id: &str, repair: bool) -> Result<ToolActionResult, String> {
    let started_at = Instant::now();
    let components = list_components_internal();
    let component = components
        .into_iter()
        .find(|item| item.id == component_id)
        .ok_or_else(|| format!("未找到组件：{component_id}"))?;

    if component.installed && !repair && component.status != "repairable" {
        return Ok(build_action_result(
            "install_component",
            "安装组件",
            true,
            format!("{} 已经处于可用状态。", component.name),
            component.summary,
            component.launch_path,
            Vec::new(),
            started_at,
        ));
    }

    let winget_id = component
        .winget_id
        .clone()
        .ok_or_else(|| "该组件暂不支持独立安装。".to_string())?;

    let mut args = vec![
        "install",
        "--id",
        winget_id.as_str(),
        "-e",
        "--accept-package-agreements",
        "--accept-source-agreements",
        "--disable-interactivity",
    ];

    if repair {
        args.push("--force");
    }

    let capture = run_command_capture("winget", &args)?;
    let refreshed_component = list_components_internal()
        .into_iter()
        .find(|item| item.id == component_id)
        .unwrap_or(component.clone());

    let manifest_path = if capture.success {
        persist_component_state(&refreshed_component)?
    } else {
        None
    };

    let log_path = write_component_log(
        &refreshed_component,
        if repair { "repair" } else { "install" },
        &format!(
            "组件：{}\n操作：{}\n结果：{}\n{}\n{}",
            refreshed_component.name,
            if repair { "修复" } else { "安装" },
            if capture.success { "成功" } else { "失败" },
            refreshed_component.summary,
            format_process_details(&capture)
        ),
    )?;

    let summary = if capture.success {
        if refreshed_component.id == "capture-plus" {
            "Snipaste 安装完成，开启后按 F1 截图，按 F3 贴图。".to_string()
        } else if refreshed_component.id == "image-viewer" {
            "图片查看器已安装完成，现在可以快速打开图片。".to_string()
        } else if refreshed_component.id == "file-converter" {
            "File Converter 已安装完成，现在右键就能直接转换文件。".to_string()
        } else if refreshed_component.id == "koodo-reader" {
            "Koodo Reader 已安装完成，现在可以直接导入和阅读电子书。".to_string()
        } else if repair {
            format!("{} 已修复完成。", refreshed_component.name)
        } else {
            format!("{} 安装完成。", refreshed_component.name)
        }
    } else {
        if repair {
            format!("{} 修复失败。", refreshed_component.name)
        } else {
            format!("{} 安装失败。", refreshed_component.name)
        }
    };

    Ok(build_action_result(
        if repair {
            "repair_component"
        } else {
            "install_component"
        },
        if repair {
            "修复组件"
        } else {
            "安装组件"
        },
        capture.success,
        summary,
        format!(
            "{}\n\n组件状态文件：{}\n组件日志：{}",
            format_process_details(&capture),
            manifest_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "未写入".to_string()),
            log_path.display()
        ),
        Some(log_path.to_string_lossy().to_string()),
        vec![String::from(
            "如果安装过程中弹出系统确认，请允许安装继续执行。",
        )],
        started_at,
    ))
}

fn install_component_internal(component_id: &str) -> Result<ToolActionResult, String> {
    run_component_install(component_id, false)
}

fn repair_component_internal(component_id: &str) -> Result<ToolActionResult, String> {
    run_component_install(component_id, true)
}

fn uninstall_component_internal(component_id: &str) -> Result<ToolActionResult, String> {
    let started_at = Instant::now();
    let components = list_components_internal();
    let component = components
        .into_iter()
        .find(|item| item.id == component_id)
        .ok_or_else(|| format!("未找到组件：{component_id}"))?;

    let winget_id = component
        .winget_id
        .clone()
        .ok_or_else(|| "该组件暂不支持卸载。".to_string())?;

    let capture = run_command_capture(
        "winget",
        &[
            "uninstall",
            "--id",
            &winget_id,
            "-e",
            "--accept-source-agreements",
            "--disable-interactivity",
        ],
    )?;

    if capture.success {
        remove_component_state(&component)?;
    }
    let log_path = write_component_log(
        &component,
        "uninstall",
        &format!(
            "组件：{}\n操作：卸载\n结果：{}\n{}",
            component.name,
            if capture.success { "成功" } else { "失败" },
            format_process_details(&capture)
        ),
    )?;

    Ok(build_action_result(
        "uninstall_component",
        "卸载组件",
        capture.success,
        if capture.success {
            format!("{} 已卸载。", component.name)
        } else {
            format!("{} 卸载失败。", component.name)
        },
        format!(
            "{}\n\n组件日志：{}",
            format_process_details(&capture),
            log_path.display()
        ),
        Some(log_path.to_string_lossy().to_string()),
        vec![String::from("部分组件卸载后可能还需要手动关闭相关进程。")],
        started_at,
    ))
}

fn launch_component_internal(component_id: &str) -> Result<ToolActionResult, String> {
    let started_at = Instant::now();
    // This is a portable WinGet package. Resolve and launch it directly so opening it
    // never waits for a full winget inventory refresh or gets mistaken for repair.
    if component_id == "context-menu-manager" {
        let mut paths = context_menu_manager_detect_paths();
        let root = local_app_data_dir().map(|p| p.join("Microsoft/WinGet/Packages"));
        if let Some(root) = root {
            paths.extend(portable_package_executables(
                &root,
                "BluePointLilac.ContextMenuManager",
                &[
                    "ContextMenuManager.exe".into(),
                    "ContextMenuManager.NET.4.0.exe".into(),
                ],
            ));
        }
        if let Some(path) = find_first_existing_path(&paths) {
            spawn_detached_path(&path, &[])?;
            return Ok(build_action_result(
                "launch_component",
                "启动组件",
                true,
                "右键菜单管理已打开。",
                format!("执行文件：{}", path.display()),
                Some(path.to_string_lossy().into()),
                Vec::new(),
                started_at,
            ));
        }
    }
    let components = list_components_internal();
    let component = components
        .into_iter()
        .find(|item| item.id == component_id)
        .ok_or_else(|| format!("未找到组件：{component_id}"))?;

    if let Some(launch_path) = component.launch_path.clone() {
        let args = component.launch_arguments.clone().unwrap_or_default();
        spawn_detached_path(Path::new(&launch_path), &args)?;
        let log_path = write_component_log(
            &component,
            "launch",
            &format!(
                "组件：{}\n操作：启动\n结果：成功\n执行文件：{}",
                component.name, launch_path
            ),
        )?;

        let summary = if component.id == "capture-plus" {
            "Snipaste 已启动。按 F1 截图，按 F3 贴图。".to_string()
        } else if component.id == "image-viewer" {
            "图片查看器已启动。现在可以直接拖图进去查看。".to_string()
        } else if component.id == "file-converter" {
            "File Converter 设置已打开，可以直接调整右键转换预设。".to_string()
        } else if component.id == "koodo-reader" {
            "Koodo Reader 已启动，现在可以直接导入电子书。".to_string()
        } else {
            format!("已启动 {}。", component.name)
        };

        Ok(build_action_result(
            "launch_component",
            "启动组件",
            true,
            summary,
            format!("执行文件：{launch_path}\n组件日志：{}", log_path.display()),
            Some(log_path.to_string_lossy().to_string()),
            Vec::new(),
            started_at,
        ))
    } else if component.installed || component.status == "repairable" {
        Ok(build_action_result(
            "launch_component",
            "启动组件",
            false,
            "已有安装记录，但未找到启动文件。",
            component.summary,
            component.install_dir,
            vec![String::from("请检查安装位置；未找到入口不代表软件损坏。")],
            started_at,
        ))
    } else if let Some(homepage) = component.homepage.clone() {
        Ok(execute_open_target(
            &homepage,
            "打开组件主页",
            "launch_component",
        ))
    } else {
        Ok(build_action_result(
            "launch_component",
            "启动组件",
            false,
            format!("{} 当前没有可启动入口。", component.name),
            component.summary,
            None,
            Vec::new(),
            started_at,
        ))
    }
}

fn disable_component_internal(component_id: &str) -> Result<ToolActionResult, String> {
    let started_at = Instant::now();

    let (process_name, summary) = match component_id {
        "capture-plus" => ("Snipaste.exe", "截图增强已关闭，已恢复系统默认截图。"),
        "clash-verge-rev" => ("clash-verge.exe", "Clash Verge Rev 已关闭。"),
        _ => {
            return Err(format!("该组件暂不支持关闭：{component_id}"));
        }
    };

    let capture = run_command_capture("taskkill", &["/IM", process_name, "/F"])?;
    let component = list_components_internal()
        .into_iter()
        .find(|item| item.id == component_id)
        .ok_or_else(|| format!("未找到组件：{component_id}"))?;
    let log_path = write_component_log(
        &component,
        "disable",
        &format!(
            "组件：{}\n操作：关闭\n结果：{}\n{}",
            component.name,
            if capture.success { "成功" } else { "失败" },
            format_process_details(&capture)
        ),
    )?;

    Ok(build_action_result(
        "disable_component",
        "关闭组件",
        capture.success,
        if capture.success {
            summary.to_string()
        } else {
            format!("未能关闭 {process_name}。")
        },
        format!(
            "{}\n\n组件日志：{}",
            format_process_details(&capture),
            log_path.display()
        ),
        Some(log_path.to_string_lossy().to_string()),
        Vec::new(),
        started_at,
    ))
}

fn scan_storage_hotspots_internal() -> Vec<StorageHotspot> {
    let mut hotspots = Vec::new();
    let mut roots = Vec::new();

    if let Some(path) = downloads_dir() {
        roots.push(("下载", path));
    }
    if let Some(path) = desktop_dir() {
        roots.push(("桌面", path));
    }
    roots.push(("文档", documents_dir()));
    if let Some(path) = videos_dir() {
        roots.push(("视频", path));
    }
    if let Some(path) = pictures_dir() {
        roots.push(("图片", path));
    }

    for (source, root) in roots {
        if !root.exists() {
            continue;
        }

        let entries = match fs::read_dir(&root) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.filter_map(|entry| entry.ok()) {
            let path = entry.path();
            let size_bytes = calculate_path_size(&path);
            if size_bytes == 0 {
                continue;
            }

            let label = path
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string());

            hotspots.push(StorageHotspot {
                id: path.to_string_lossy().to_string(),
                label,
                path: path.to_string_lossy().to_string(),
                source: source.to_string(),
                size_bytes,
                item_count: if path.is_dir() {
                    count_immediate_children(&path)
                } else {
                    1
                },
            });
        }
    }

    hotspots.sort_by(|left, right| {
        right
            .size_bytes
            .cmp(&left.size_bytes)
            .then(left.source.cmp(&right.source))
            .then(left.label.cmp(&right.label))
    });
    hotspots.truncate(16);
    hotspots
}

fn get_third_party_notices_internal() -> Vec<ThirdPartyNotice> {
    list_components_internal()
        .into_iter()
        .filter(|item| item.kind != "built-in")
        .filter_map(|item| {
            let source_url = item.source_url.clone().or_else(|| item.homepage.clone())?;
            let source_label = item
                .source_label
                .clone()
                .unwrap_or_else(|| "官方来源".to_string());
            let license_name = item
                .license_name
                .clone()
                .unwrap_or_else(|| "请查看官网说明".to_string());

            Some(ThirdPartyNotice {
                id: item.id,
                name: item.name,
                version: item.version.unwrap_or_else(|| "当前可用版本".to_string()),
                source_label,
                source_url,
                license_name,
                license_url: item.license_url,
                notes: item.summary,
            })
        })
        .collect()
}

fn format_process_details(capture: &ProcessCapture) -> String {
    let mut parts = Vec::new();

    if !capture.stdout.trim().is_empty() {
        parts.push(capture.stdout.trim().to_string());
    }

    if !capture.stderr.trim().is_empty() {
        parts.push(capture.stderr.trim().to_string());
    }

    if parts.is_empty() {
        "没有产生额外输出。".to_string()
    } else {
        parts.join("\n\n")
    }
}

fn format_bytes(bytes: u64) -> String {
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;

    if bytes as f64 >= GIB {
        format!("{:.2} GB", bytes as f64 / GIB)
    } else {
        format!("{:.2} MB", bytes as f64 / MIB)
    }
}

fn calculate_path_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }

    if path.is_file() {
        return fs::metadata(path).map(|item| item.len()).unwrap_or(0);
    }

    WalkDir::new(path)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.metadata().ok())
        .filter(|metadata| metadata.is_file())
        .map(|metadata| metadata.len())
        .sum()
}

fn count_immediate_children(path: &Path) -> u64 {
    fs::read_dir(path)
        .map(|entries| entries.filter_map(|entry| entry.ok()).count() as u64)
        .unwrap_or(0)
}

fn command_exists(program: &str) -> bool {
    run_command_capture("where.exe", &[program])
        .map(|capture| capture.success)
        .unwrap_or(false)
}

fn winget_package_installed(package_id: &str) -> bool {
    if !command_exists("winget") {
        return false;
    }

    run_command_capture(
        "winget",
        &[
            "list",
            "--id",
            package_id,
            "-e",
            "--accept-source-agreements",
            "--disable-interactivity",
        ],
    )
    .map(|capture| {
        capture.success
            && !capture.stdout.contains("No installed package found")
            && !capture.stdout.contains("没有已安装的程序包")
    })
    .unwrap_or(false)
}

fn unix_timestamp_slug() -> String {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs().to_string(),
        Err(_) => "0".to_string(),
    }
}

fn run_command_capture(program: &str, args: &[&str]) -> Result<ProcessCapture, String> {
    let output = Command::new(program)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|error| format!("Failed to run {program}: {error}"))?;

    Ok(ProcessCapture {
        exit_code: output.status.code(),
        success: output.status.success(),
        stdout: decode_command_output(&output.stdout),
        stderr: decode_command_output(&output.stderr),
    })
}

fn decode_command_output(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }

    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.trim().to_string();
    }

    let (decoded, _, _) = GBK.decode(bytes);
    decoded.trim().to_string()
}

fn with_powershell_utf8(script: &str) -> String {
    format!(
        "$OutputEncoding = [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)\n{script}"
    )
}

fn run_powershell_json(script: &str) -> Result<String, String> {
    let wrapped_script = with_powershell_utf8(script);
    let capture = run_command_capture(
        "powershell.exe",
        &[
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &wrapped_script,
        ],
    )?;

    if capture.success {
        Ok(capture.stdout)
    } else {
        Err(format_process_details(&capture))
    }
}

fn spawn_detached(program: &str, args: &[String]) -> Result<(), String> {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|error| format!("Failed to launch {program}: {error}"))?;

    Ok(())
}

fn spawn_detached_path(executable: &Path, args: &[String]) -> Result<(), String> {
    component_launcher::launch(executable, args)
}

fn build_action_result(
    action_id: &str,
    title: &str,
    success: bool,
    summary: impl Into<String>,
    details: impl Into<String>,
    output_path: Option<String>,
    warnings: Vec<String>,
    started_at: Instant,
) -> ToolActionResult {
    ToolActionResult {
        action_id: action_id.to_string(),
        title: title.to_string(),
        success,
        summary: summary.into(),
        details: details.into(),
        duration_ms: started_at.elapsed().as_millis() as u64,
        output_path,
        warnings,
    }
}

fn execute_open_target(target: &str, title: &str, action_id: &str) -> ToolActionResult {
    let started_at = Instant::now();
    match spawn_detached("explorer.exe", &[target.to_string()]) {
        Ok(()) => build_action_result(
            action_id,
            title,
            true,
            format!("已打开目标：{target}"),
            "已交给 Windows 资源管理器处理。",
            None,
            Vec::new(),
            started_at,
        ),
        Err(error) => build_action_result(
            action_id,
            title,
            false,
            "目标无法打开。",
            error,
            None,
            Vec::new(),
            started_at,
        ),
    }
}

fn execute_launch_capture(use_helper: bool) -> ToolActionResult {
    let started_at = Instant::now();

    if let Some(snipaste_path) = if use_helper {
        find_first_existing_path(&snipaste_detect_paths())
    } else {
        None
    } {
        match spawn_detached_path(&snipaste_path, &Vec::new()) {
            Ok(()) => {
                return build_action_result(
                    "launch_capture",
                    "截图",
                    true,
                    "Snipaste 已启动。按 F1 截图，按 F3 贴图。",
                    format!(
                        "组件：Snipaste\n执行文件：{}\n快捷键：F1 截图，F3 贴图",
                        snipaste_path.display()
                    ),
                    Some(snipaste_path.to_string_lossy().to_string()),
                    Vec::new(),
                    started_at,
                )
            }
            Err(error) => {
                return build_action_result(
                    "launch_capture",
                    "截图",
                    false,
                    "检测到 Snipaste，但启动失败。",
                    error,
                    Some(snipaste_path.to_string_lossy().to_string()),
                    Vec::new(),
                    started_at,
                )
            }
        }
    }

    match spawn_detached("explorer.exe", &[String::from("ms-screenclip:")]) {
        Ok(()) => build_action_result(
            "launch_capture",
            "截图",
            true,
            "已打开系统截图。",
            "当前使用 Windows 自带截图。你也可以在效率页开启 Snipaste 增强截图。",
            None,
            Vec::new(),
            started_at,
        ),
        Err(error) => build_action_result(
            "launch_capture",
            "截图",
            false,
            "系统截图工具无法启动。",
            error,
            None,
            Vec::new(),
            started_at,
        ),
    }
}

fn execute_tool_action(action_id: &str, capture_helper_enabled: bool) -> ToolActionResult {
    match action_id {
        "launch_capture" => execute_launch_capture(capture_helper_enabled),
        "open_apps_features" => {
            execute_open_target("ms-settings:appsfeatures", "应用管理", "open_apps_features")
        }
        "open_notifications" => execute_open_target(
            "ms-settings:notifications",
            "通知净化",
            "open_notifications",
        ),
        "open_windows_update" => execute_open_target(
            "ms-settings:windowsupdate",
            "更新中心",
            "open_windows_update",
        ),
        _ => build_action_result(
            action_id,
            "未知动作",
            false,
            "请求的动作尚未实现。",
            format!("未知动作 ID：{action_id}"),
            None,
            Vec::new(),
            Instant::now(),
        ),
    }
}

fn get_system_snapshot_internal() -> Result<SystemSnapshot, String> {
    let script = r#"
$ErrorActionPreference = 'Stop'
$cpu = Get-CimInstance Win32_Processor | Select-Object -First 1 Name, LoadPercentage, NumberOfCores, NumberOfLogicalProcessors
$os = Get-CimInstance Win32_OperatingSystem | Select-Object -First 1 Caption, Version, BuildNumber, TotalVisibleMemorySize, FreePhysicalMemory, CSName
$gpu = Get-CimInstance Win32_VideoController | Select-Object -First 1 Name, AdapterRAM
$gpuName = if ($gpu) { $gpu.Name } else { $null }
$gpuMemoryMb = $null

if (Get-Command nvidia-smi -ErrorAction SilentlyContinue) {
  $nvidiaLine = & nvidia-smi --query-gpu=name,memory.total --format=csv,noheader 2>$null | Select-Object -First 1
  if ($nvidiaLine) {
    $parts = $nvidiaLine -split ','
    if ($parts.Count -ge 2) {
      $gpuName = $parts[0].Trim()
      $memoryValue = [regex]::Match($parts[1], '\d+').Value
      if ($memoryValue) {
        $gpuMemoryMb = [uint64]$memoryValue
      }
    }
  }
}

if (-not $gpuMemoryMb -and $gpu -and $gpu.AdapterRAM) {
  $gpuMemoryMb = [uint64][math]::Round($gpu.AdapterRAM / 1MB, 0)
}

$cpuLoad = if ($cpu.LoadPercentage -eq $null) {
  [uint64]0
} else {
  [uint64]$cpu.LoadPercentage
}

$memoryUsagePercent = if ($os.TotalVisibleMemorySize -gt 0) {
  [uint64][math]::Round((($os.TotalVisibleMemorySize - $os.FreePhysicalMemory) / $os.TotalVisibleMemorySize) * 100, 0)
} else {
  [uint64]0
}

$primaryNetwork = Get-NetAdapter |
  Where-Object Status -eq 'Up' |
  Where-Object InterfaceDescription -notmatch 'Hyper-V|Virtual|VPN|Tunnel|TAP|Loopback|Miniport' |
  Sort-Object LinkSpeed -Descending |
  Select-Object -First 1 Name, InterfaceDescription, LinkSpeed

if (-not $primaryNetwork) {
  $primaryNetwork = Get-NetAdapter |
    Where-Object Status -eq 'Up' |
    Sort-Object LinkSpeed -Descending |
    Select-Object -First 1 Name, InterfaceDescription, LinkSpeed
}

[PSCustomObject]@{
  hostName = $os.CSName
  osName = $os.Caption
  osVersion = $os.Version
  osBuild = $os.BuildNumber
  cpuName = $cpu.Name
  cpuLoad = $cpuLoad
  cpuCores = [uint64]$cpu.NumberOfCores
  logicalCores = [uint64]$cpu.NumberOfLogicalProcessors
  memoryTotalMb = [uint64][math]::Round($os.TotalVisibleMemorySize / 1024, 0)
  memoryUsedMb = [uint64][math]::Round(($os.TotalVisibleMemorySize - $os.FreePhysicalMemory) / 1024, 0)
  memoryUsagePercent = $memoryUsagePercent
  gpuName = $gpuName
  gpuMemoryMb = $gpuMemoryMb
  networkName = if ($primaryNetwork) { $primaryNetwork.Name } else { $null }
  networkDescription = if ($primaryNetwork) { $primaryNetwork.InterfaceDescription } else { $null }
  networkLinkSpeed = if ($primaryNetwork) { [string]$primaryNetwork.LinkSpeed } else { $null }
  collectedAt = (Get-Date).ToString('o')
} | ConvertTo-Json -Compress
"#;

    let raw_snapshot = run_powershell_json(script)?;
    serde_json::from_str(&raw_snapshot)
        .map_err(|error| format!("Failed to parse system snapshot: {error}"))
}

#[tauri::command]
async fn get_system_snapshot() -> Result<SystemSnapshot, String> {
    tauri::async_runtime::spawn_blocking(get_system_snapshot_internal)
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn list_components() -> Result<Vec<ComponentManifest>, String> {
    tauri::async_runtime::spawn_blocking(list_components_internal)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn get_third_party_notices() -> Result<Vec<ThirdPartyNotice>, String> {
    tauri::async_runtime::spawn_blocking(get_third_party_notices_internal)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_component_logs(component_id: String) -> Result<Vec<String>, String> {
    let log_dir = component_logs_root().join(&component_id);
    if !log_dir.exists() {
        return Ok(Vec::new());
    }

    let mut logs = fs::read_dir(&log_dir)
        .map_err(|error| format!("无法读取组件日志目录：{error}"))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path().to_string_lossy().to_string())
        .collect::<Vec<_>>();

    logs.sort();
    Ok(logs)
}

#[tauri::command]
async fn run_tool_action(
    action_id: String,
    capture_helper_enabled: Option<bool>,
) -> Result<ToolActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        execute_tool_action(&action_id, capture_helper_enabled.unwrap_or(false))
    })
    .await
    .map_err(|error| format!("执行动作任务失败：{error}"))
}

#[tauri::command]
async fn launch_component(component_id: String) -> Result<ToolActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _task = component_updates::begin_component_task(&component_id)?;
        launch_component_internal(&component_id)
    })
    .await
    .map_err(|error| format!("启动组件任务失败：{error}"))?
}

#[tauri::command]
async fn manage_component(
    component_id: String,
    operation: String,
) -> Result<ToolActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _task = component_updates::begin_component_task(&component_id)?;
        match operation.as_str() {
            "install" => install_component_internal(&component_id),
            "repair" => repair_component_internal(&component_id),
            "uninstall" => uninstall_component_internal(&component_id),
            "update" => component_updates::update_component(&component_id),
            "disable" => disable_component_internal(&component_id),
            _ => Err(format!("未知组件操作：{operation}")),
        }
    })
    .await
    .map_err(|error| format!("组件管理任务失败：{error}"))?
}

#[tauri::command]
async fn open_target(target: String) -> Result<ToolActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        execute_open_target(&target, "打开目标", "open_target")
    })
    .await
    .map_err(|error| format!("打开目标任务失败：{error}"))
}

#[tauri::command]
async fn scan_storage_hotspots() -> Result<Vec<StorageHotspot>, String> {
    tauri::async_runtime::spawn_blocking(scan_storage_hotspots_internal)
        .await
        .map_err(|error| format!("扫描空间热点失败：{error}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(cleaning::CleaningState::default())
        .manage(file_management::FileState::default())
        .setup(|app| {
            popups::start_worker();
            if let Some(window) = app.get_webview_window("main") {
                if let Some(monitor) = window.current_monitor()? {
                    let scale = monitor.scale_factor();
                    let size = monitor.work_area().size;
                    let width = (size.width as f64 / scale - 32.0).min(1280.0);
                    let height = (size.height as f64 / scale - 32.0).min(820.0);
                    window.set_min_size(Some(tauri::LogicalSize::new(
                        width.min(760.0),
                        height.min(520.0),
                    )))?;
                    window.set_size(tauri::LogicalSize::new(width, height))?;
                    window.center()?;
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_system_snapshot,
            list_components,
            component_updates::check_component_updates,
            get_third_party_notices,
            get_component_logs,
            run_tool_action,
            launch_component,
            manage_component,
            open_target,
            scan_storage_hotspots,
            cleaning::scan_cleaning,
            cleaning::clean_selected,
            cleaning::cleaning_files,
            windows_settings::get_windows_settings,
            windows_settings::set_windows_setting,
            windows_settings::restore_windows_setting,
            management::list_processes,
            application_icons::get_application_icon,
            application_icons::get_file_icon,
            management::list_application_windows,
            popups::list_popup_rules,
            popups::add_popup_rule,
            popups::remove_popup_rule,
            management::end_process,
            management::list_startup_items,
            management::set_startup_item,
            network::network_scan,
            network::network_repair,
            network::network_backups,
            network::network_backup,
            network::network_restore,
            network::network_tools,
            network::network_export_report,
            network::speed::network_speedtest,
            network::speed::network_cancel_speedtest,
            management::health_check,
            management::get_power_plan,
            management::set_power_plan,
            management::repair_taskbar,
            system_repair::repair_windows,
            file_management::personal_folders,
            file_management::scan_personal_files,
            file_management::cancel_file_scan,
            file_management::file_scan_drives,
            file_management::storage_drives,
            file_management::recycle_selected_files
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

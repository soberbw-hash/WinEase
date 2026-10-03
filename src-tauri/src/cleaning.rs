use crate::{build_action_result, format_bytes, local_app_data_dir, ToolActionResult};
use serde::Serialize;
use std::collections::BTreeMap;
use std::{
    fs,
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{Duration, Instant, SystemTime},
};
use walkdir::WalkDir;

const MIN_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const SCAN_TTL: Duration = Duration::from_secs(15 * 60);
const REPARSE_POINT: u32 = 0x400;
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Default)]
pub struct CleaningState(Mutex<Option<Scan>>);
struct Candidate {
    path: PathBuf,
    category: &'static str,
    size: u64,
    modified: SystemTime,
    min_age: Duration,
}
struct Scan {
    id: String,
    roots: Vec<PathBuf>,
    created: Instant,
    files: Vec<Candidate>,
    skipped: u64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    id: &'static str,
    label: &'static str,
    size_bytes: u64,
    file_count: u64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleaningScan {
    scan_id: String,
    categories: Vec<Category>,
    skipped_entries: u64,
    groups: Vec<CleaningGroup>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleaningGroup {
    id: String,
    label: String,
    category: &'static str,
    path: String,
    size_bytes: u64,
    file_count: u64,
    recommended: bool,
    icon_target: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleaningFile {
    id: usize,
    name: String,
    path: String,
    size_bytes: u64,
}
#[derive(Serialize)]
pub struct CleaningFiles {
    files: Vec<CleaningFile>,
    total: usize,
}
fn group_key(scan: &Scan, file: &Candidate) -> String {
    let root = scan
        .roots
        .iter()
        .enumerate()
        .filter(|(_, root)| file.path.starts_with(root))
        .max_by_key(|(_, root)| root.components().count())
        .map(|(i, _)| i)
        .unwrap_or(0);
    format!("{}:{root}", file.category)
}
fn group_label(path: &Path, category: &str) -> String {
    let text = path.to_string_lossy().to_lowercase();
    let app = [
        ("google", "Chrome"),
        ("microsoft\\edge", "Edge"),
        ("brave", "Brave"),
        ("vivaldi", "Vivaldi"),
        ("firefox", "Firefox"),
        ("nvidia", "NVIDIA"),
        ("npm-cache", "Node.js"),
        ("pip", "Python"),
        ("nuget", "NuGet"),
        ("discord", "Discord"),
        ("slack", "Slack"),
        ("teams", "Teams"),
        ("steam", "Steam"),
        ("doubao", "豆包"),
        ("todesk", "ToDesk"),
        ("chatgpt", "ChatGPT"),
        ("\\code\\", "VS Code"),
    ]
    .into_iter()
    .find(|(key, _)| text.contains(key))
    .map(|(_, name)| name);
    let kind = match category {
        "logs" => "日志文件",
        "caches" => {
            if text.contains("explorer") {
                "缩略图缓存"
            } else {
                "着色器缓存"
            }
        }
        "browser" => "网页缓存",
        "applications" => "运行缓存",
        "crashes" => "错误报告",
        "downloads-cache" => "下载缓存",
        _ => "临时文件",
    };
    app.map(|app| format!("{app} · {kind}"))
        .unwrap_or_else(|| kind.into())
}
fn cleaning_icon_target(label: &str) -> Option<String> {
    let app = label.split(" · ").next()?;
    let relative: &[&str] = match app {
        "Chrome" => &[r"Google\Chrome\Application\chrome.exe"],
        "Edge" => &[r"Microsoft\Edge\Application\msedge.exe"],
        "Brave" => &[r"BraveSoftware\Brave-Browser\Application\brave.exe"],
        "Vivaldi" => &[r"Vivaldi\Application\vivaldi.exe"],
        "Firefox" => &[r"Mozilla Firefox\firefox.exe"],
        "VS Code" => &[
            r"Microsoft VS Code\Code.exe",
            r"Programs\Microsoft VS Code\Code.exe",
        ],
        "Node.js" => &[r"nodejs\node.exe"],
        "Steam" => &[r"Steam\steam.exe"],
        _ => return None,
    };
    for root in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
        if let Some(root) = std::env::var_os(root) {
            for relative in relative {
                let path = PathBuf::from(&root).join(relative);
                if path.is_file() && no_reparse_ancestors(&path) {
                    return Some(path.to_string_lossy().into());
                }
            }
        }
    }
    None
}
impl Scan {
    fn groups(&self) -> Vec<CleaningGroup> {
        let mut groups: BTreeMap<String, CleaningGroup> = BTreeMap::new();
        for file in &self.files {
            let id = group_key(self, file);
            let index: usize = id.rsplit(':').next().unwrap().parse().unwrap();
            let root = &self.roots[index];
            let group = groups.entry(id.clone()).or_insert_with(|| CleaningGroup {
                id,
                label: group_label(root, file.category),
                category: file.category,
                path: root.to_string_lossy().into(),
                size_bytes: 0,
                file_count: 0,
                recommended: !matches!(file.category, "downloads-cache" | "caches" | "crashes"),
                icon_target: cleaning_icon_target(&group_label(root, file.category)),
            });
            group.size_bytes = group.size_bytes.saturating_add(file.size);
            group.file_count += 1;
        }
        let mut groups: Vec<_> = groups.into_values().collect();
        groups.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
        groups
    }
}
#[tauri::command]
pub fn cleaning_files(
    scan_id: String,
    group_id: String,
    group_ids: Option<Vec<String>>,
    offset: usize,
    state: tauri::State<'_, CleaningState>,
) -> Result<CleaningFiles, String> {
    let lock = state.0.lock().map_err(|_| "清理任务状态异常")?;
    let scan = lock.as_ref().ok_or("请重新扫描")?;
    if scan.id != scan_id || scan.created.elapsed() > SCAN_TTL {
        return Err("扫描结果已失效，请重新扫描".into());
    }
    let ids: std::collections::HashSet<_> = group_ids
        .unwrap_or_else(|| vec![group_id])
        .into_iter()
        .collect();
    inspect_files(scan, ids, offset)
}
fn inspect_files(
    scan: &Scan,
    ids: std::collections::HashSet<String>,
    offset: usize,
) -> Result<CleaningFiles, String> {
    let groups = scan.groups();
    if ids.is_empty() || ids.iter().any(|id| !groups.iter().any(|g| &g.id == id)) {
        return Err("清理项目无效".into());
    }
    let items: Vec<_> = scan
        .files
        .iter()
        .enumerate()
        .filter(|(_, f)| ids.contains(&group_key(scan, f)))
        .collect();
    Ok(CleaningFiles {
        total: items.len(),
        files: items
            .into_iter()
            .skip(offset)
            .take(100)
            .map(|(id, f)| CleaningFile {
                id,
                name: f
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into(),
                path: f.path.to_string_lossy().into(),
                size_bytes: f.size,
            })
            .collect(),
    })
}

pub(super) fn no_reparse_ancestors(path: &Path) -> bool {
    path.ancestors().all(|ancestor| {
        fs::symlink_metadata(ancestor)
            .map(|m| m.file_attributes() & REPARSE_POINT == 0)
            .unwrap_or(false)
    })
}
fn eligible_age(metadata: &fs::Metadata, now: SystemTime, min_age: Duration) -> bool {
    metadata.is_file()
        && metadata.file_attributes() & REPARSE_POINT == 0
        && metadata
            .modified()
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age >= min_age)
}
fn scan_root(root: &Path) -> Result<Scan, String> {
    scan_root_age(root, MIN_AGE)
}
fn scan_root_age(root: &Path, min_age: Duration) -> Result<Scan, String> {
    if !root.is_absolute() || !no_reparse_ancestors(root) {
        return Err("临时目录不存在或包含链接，已停止扫描。".into());
    }
    let root = fs::canonicalize(root).map_err(|e| format!("无法读取临时目录：{e}"))?;
    let now = SystemTime::now();
    let mut files = Vec::new();
    let mut skipped = 0;
    let mut linked = 0;
    let walker = WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            let safe = fs::symlink_metadata(entry.path())
                .map(|m| m.file_attributes() & REPARSE_POINT == 0)
                .unwrap_or(false);
            if !safe {
                linked += 1;
            }
            safe
        });
    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        if entry.file_type().is_dir() {
            continue;
        }
        match fs::symlink_metadata(entry.path()) {
            Ok(metadata) if eligible_age(&metadata, now, min_age) => {
                let category = if entry
                    .path()
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("log"))
                {
                    "logs"
                } else {
                    "temporary"
                };
                files.push(Candidate {
                    path: entry.path().to_owned(),
                    category,
                    size: metadata.len(),
                    modified: metadata.modified().unwrap(),
                    min_age,
                });
            }
            _ => skipped += 1,
        }
    }
    Ok(Scan {
        id: format!(
            "{}-{}",
            crate::unix_timestamp_slug(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ),
        roots: vec![root],
        created: Instant::now(),
        files,
        skipped: skipped + linked,
    })
}
impl Scan {
    fn summary(&self) -> CleaningScan {
        let categories = [
            ("temporary", "临时文件"),
            ("logs", "临时日志"),
            ("caches", "图形与缩略图缓存"),
            ("browser", "浏览器网页缓存"),
            ("applications", "应用运行缓存"),
            ("crashes", "崩溃转储与错误报告"),
            ("downloads-cache", "下载与构建缓存"),
        ]
        .into_iter()
        .map(|(id, label)| {
            let files = self.files.iter().filter(|file| file.category == id);
            let (size_bytes, file_count) =
                files.fold((0, 0), |(size, count), file| (size + file.size, count + 1));
            Category {
                id,
                label,
                size_bytes,
                file_count,
            }
        })
        .collect();
        CleaningScan {
            scan_id: self.id.clone(),
            categories,
            skipped_entries: self.skipped,
            groups: self.groups(),
        }
    }
}
fn take_scan(state: &CleaningState, scan_id: &str, categories: &[String]) -> Result<Scan, String> {
    let mut current = state.0.lock().map_err(|_| "清理任务状态异常".to_string())?;
    let scan = current.as_ref().ok_or("请先扫描临时文件。")?;
    if scan.id != scan_id || scan.created.elapsed() > SCAN_TTL {
        return Err("扫描结果已失效，请重新扫描。".into());
    }
    if categories.is_empty()
        || categories.iter().any(|id| {
            !scan.summary().categories.iter().any(|c| c.id == id)
                && !scan.groups().iter().any(|g| &g.id == id)
        })
    {
        return Err("请选择有效的清理项目。".into());
    }
    Ok(current.take().unwrap())
}
fn clean_scan(scan: Scan, categories: &[String]) -> ToolActionResult {
    let started = Instant::now();
    let mut freed = 0;
    let mut removed = 0;
    let mut skipped = 0;
    for file in scan.files.iter().filter(|file| {
        categories
            .iter()
            .any(|id| id == file.category || *id == group_key(&scan, file))
    }) {
        if !scan.roots.iter().any(|root| file.path.starts_with(root))
            || !no_reparse_ancestors(&file.path)
        {
            skipped += 1;
            continue;
        }
        let metadata = match fs::symlink_metadata(&file.path) {
            Ok(m) => m,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        if !eligible_age(&metadata, SystemTime::now(), file.min_age)
            || metadata.len() != file.size
            || metadata.modified().ok() != Some(file.modified)
        {
            skipped += 1;
            continue;
        }
        // Do not delete a cache file still held open by a browser/application writer.
        let _exclusive = match fs::OpenOptions::new()
            .read(true)
            .share_mode(4)
            .open(&file.path)
        {
            Ok(file) => file,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        match fs::remove_file(&file.path) {
            Ok(()) => {
                freed += file.size;
                removed += 1;
            }
            Err(_) => skipped += 1,
        }
    }
    build_action_result("clean_selected", "临时文件清理", skipped == 0,
        format!("已清理 {removed} 个文件，释放 {}。", format_bytes(freed)),
        format!("已删除：{removed} 个文件\n文件大小合计：{}\n执行时跳过：{skipped} 个已变化、被占用或链接项目\n保留空目录", format_bytes(freed)),
        None, if skipped > 0 { vec!["部分文件已变化或无法删除，可重新扫描。".into()] } else { Vec::new() }, started)
}

#[tauri::command]
pub async fn scan_cleaning(state: tauri::State<'_, CleaningState>) -> Result<CleaningScan, String> {
    let root = local_app_data_dir()
        .ok_or("无法定位当前用户临时目录。")?
        .join("Temp");
    let scan = tauri::async_runtime::spawn_blocking(move || {
        let mut scan = scan_root(&root)?;
        let local = root.parent().ok_or("无法定位用户目录")?;
        for path in [
            local.join("D3DSCache"),
            local.join(r"NVIDIA\DXCache"),
            local.join(r"NVIDIA\GLCache"),
            local.join(r"Microsoft\Windows\Explorer"),
        ] {
            if !path.exists() {
                continue;
            }
            match scan_root_age(&path, Duration::from_secs(86400)) {
                Ok(mut cache) => {
                    if path.ends_with(r"Microsoft\Windows\Explorer") {
                        cache.files.retain(|file| {
                            file.path.file_name().is_some_and(|name| {
                                name.to_string_lossy().starts_with("thumbcache_")
                                    && file
                                        .path
                                        .extension()
                                        .is_some_and(|ext| ext.eq_ignore_ascii_case("db"))
                            })
                        });
                    }
                    for file in &mut cache.files {
                        file.category = "caches";
                    }
                    scan.roots.extend(cache.roots);
                    scan.files.extend(cache.files);
                    scan.skipped += cache.skipped;
                }
                Err(_) => scan.skipped += 1,
            }
        }
        extend_cleaners(&mut scan, local);
        Ok::<_, String>(scan)
    })
    .await
    .map_err(|e| e.to_string())??;
    let summary = scan.summary();
    *state.0.lock().map_err(|_| "清理任务状态异常".to_string())? = Some(scan);
    Ok(summary)
}
fn extend_cleaners(scan: &mut Scan, local: &Path) {
    let mut targets: Vec<(PathBuf, &'static str, Duration)> = Vec::new();
    let cache_age = Duration::from_secs(24 * 60 * 60);
    for relative in [
        r"Google\Chrome\User Data",
        r"Microsoft\Edge\User Data",
        r"BraveSoftware\Brave-Browser\User Data",
        r"Vivaldi\User Data",
    ] {
        let directory = local.join(relative);
        if let Ok(profiles) = fs::read_dir(&directory) {
            for profile in profiles.flatten() {
                let name = profile.file_name().to_string_lossy().into_owned();
                if name != "Default" && !name.starts_with("Profile ") {
                    continue;
                }
                for cache in [
                    "Cache",
                    "Code Cache",
                    "GPUCache",
                    "DawnGraphiteCache",
                    "DawnWebGPUCache",
                ] {
                    targets.push((profile.path().join(cache), "browser", cache_age));
                }
            }
        }
        targets.push((directory.join(r"ShaderCache"), "browser", cache_age));
    }
    if let Ok(profiles) = fs::read_dir(local.join(r"Mozilla\Firefox\Profiles")) {
        for profile in profiles.flatten() {
            targets.push((profile.path().join("cache2"), "browser", cache_age));
        }
    }
    if let Some(roaming) = std::env::var_os("APPDATA") {
        for app in [
            "Code",
            "discord",
            "Slack",
            "Microsoft\\Teams",
            "ChatGPT",
            "Doubao",
            "ToDesk",
        ] {
            for cache in ["Cache", "Code Cache", "GPUCache"] {
                targets.push((
                    PathBuf::from(&roaming).join(app).join(cache),
                    "applications",
                    cache_age,
                ));
            }
        }
    }
    for relative in [r"npm-cache\_cacache", r"pip\Cache", r"NuGet\v3-cache"] {
        targets.push((local.join(relative), "downloads-cache", MIN_AGE));
    }
    for relative in [
        r"Steam\htmlcache\Cache",
        r"Steam\htmlcache\Code Cache",
        r"Steam\htmlcache\GPUCache",
    ] {
        targets.push((local.join(relative), "applications", cache_age));
    }
    for relative in [
        "CrashDumps",
        r"Microsoft\Windows\WER\ReportArchive",
        r"Microsoft\Windows\WER\ReportQueue",
    ] {
        targets.push((local.join(relative), "crashes", MIN_AGE));
    }
    if let Some(windows) = std::env::var_os("SystemRoot") {
        let windows = PathBuf::from(windows);
        targets.push((windows.join("Temp"), "temporary", MIN_AGE));
        // Only rotated log files; never servicing packages or update databases.
        for directory in [r"Logs\CBS", r"Logs\DISM", r"Panther"] {
            targets.push((windows.join(directory), "logs", MIN_AGE));
        }
        targets.push((windows.join("Minidump"), "crashes", MIN_AGE));
    }
    if let Some(data) = std::env::var_os("ProgramData") {
        for folder in [
            r"Microsoft\Windows\WER\ReportArchive",
            r"Microsoft\Windows\WER\ReportQueue",
        ] {
            targets.push((PathBuf::from(&data).join(folder), "crashes", MIN_AGE));
        }
    }
    for (path, category, age) in targets {
        if !path.is_dir() {
            continue;
        }
        match scan_root_age(&path, age) {
            Ok(mut extra) => {
                if category == "logs" {
                    extra.files.retain(|f| {
                        f.path
                            .extension()
                            .is_some_and(|ext| ext.eq_ignore_ascii_case("log"))
                    });
                }
                for file in &mut extra.files {
                    file.category = category;
                }
                scan.roots.extend(extra.roots);
                scan.files.extend(extra.files);
                scan.skipped += extra.skipped;
            }
            Err(_) => scan.skipped += 1,
        }
    }
}
#[tauri::command]
pub async fn clean_selected(
    scan_id: String,
    category_ids: Vec<String>,
    state: tauri::State<'_, CleaningState>,
) -> Result<ToolActionResult, String> {
    let scan = take_scan(&state, &scan_id, &category_ids)?;
    tauri::async_runtime::spawn_blocking(move || clean_scan(scan, &category_ids))
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::process::CommandExt;
    fn old_file(root: &Path, name: &str) -> PathBuf {
        let path = root.join(name);
        fs::write(&path, b"test file").unwrap();
        let modified = filetime::FileTime::from_system_time(
            SystemTime::now() - MIN_AGE - Duration::from_secs(60),
        );
        filetime::set_file_mtime(&path, modified).unwrap();
        path
    }
    #[test]
    fn merged_detail_pages_include_all_roots_once_and_reject_unknown_roots() {
        let root = tempfile::tempdir().unwrap();
        let mut all = None;
        for (name, count) in [("Default", 60), ("Profile 1", 70)] {
            let directory = root.path().join(name);
            fs::create_dir(&directory).unwrap();
            for i in 0..count {
                old_file(&directory, &format!("{i}.cache"));
            }
            let mut scan = scan_root(&directory).unwrap();
            for file in &mut scan.files {
                file.category = "browser";
            }
            if let Some(existing) = all.as_mut() {
                let existing: &mut Scan = existing;
                existing.roots.extend(scan.roots);
                existing.files.extend(scan.files);
            } else {
                all = Some(scan);
            }
        }
        let scan = all.unwrap();
        let ids: std::collections::HashSet<_> =
            scan.groups().iter().map(|g| g.id.clone()).collect();
        let first = inspect_files(&scan, ids.clone(), 0).unwrap();
        let second = inspect_files(&scan, ids, 100).unwrap();
        assert_eq!(first.total, 130);
        assert_eq!(first.files.len(), 100);
        assert_eq!(second.files.len(), 30);
        let unique: std::collections::HashSet<_> = first
            .files
            .iter()
            .chain(second.files.iter())
            .map(|f| f.id)
            .collect();
        assert_eq!(unique.len(), 130);
        assert!(inspect_files(&scan, ["browser:unknown".into()].into(), 0).is_err());
    }
    #[test]
    fn root_groups_select_only_inspected_cache_and_keep_other_app() {
        let root = tempfile::tempdir().unwrap();
        let chrome = root.path().join("Chrome");
        let edge = root.path().join("Edge");
        fs::create_dir(&chrome).unwrap();
        fs::create_dir(&edge).unwrap();
        let first = old_file(&chrome, "a.cache");
        let second = old_file(&edge, "b.cache");
        let mut scan = scan_root(&chrome).unwrap();
        let other = scan_root(&edge).unwrap();
        scan.roots.extend(other.roots);
        scan.files.extend(other.files);
        for file in &mut scan.files {
            file.category = "browser";
        }
        let groups = scan.groups();
        assert_eq!(groups.len(), 2);
        assert_eq!(
            groups.iter().map(|g| g.size_bytes).sum::<u64>(),
            scan.files.iter().map(|f| f.size).sum::<u64>()
        );
        let first_group = group_key(&scan, &scan.files[0]);
        assert!(clean_scan(scan, &[first_group]).success);
        assert!(!first.exists());
        assert!(second.exists());
    }
    #[test]
    fn costly_rebuilds_and_diagnostic_reports_are_optional() {
        let root = tempfile::tempdir().unwrap();
        old_file(root.path(), "cache.bin");
        let mut scan = scan_root(root.path()).unwrap();
        for category in ["downloads-cache", "caches", "crashes"] {
            scan.files[0].category = category;
            assert!(!scan.groups()[0].recommended);
        }
        assert_eq!(
            group_label(
                Path::new(r"C:\Users\Test\AppData\Local\Google\Chrome\User Data\Default\Cache"),
                "browser"
            ),
            "Chrome · 网页缓存"
        );
    }
    #[test]
    fn scan_excludes_recent_files() {
        let root = tempfile::tempdir().unwrap();
        old_file(root.path(), "old.tmp");
        fs::write(root.path().join("new.tmp"), "keep").unwrap();
        let scan = scan_root(root.path()).unwrap();
        assert_eq!(scan.files.len(), 1);
        assert_eq!(scan.skipped, 1);
    }
    #[test]
    fn cache_age_does_not_relax_temporary_file_protection() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("cache.bin");
        fs::write(&path, "cache").unwrap();
        filetime::set_file_mtime(
            &path,
            filetime::FileTime::from_system_time(
                SystemTime::now() - Duration::from_secs(2 * 86400),
            ),
        )
        .unwrap();
        assert!(scan_root(root.path()).unwrap().files.is_empty());
        assert_eq!(
            scan_root_age(root.path(), Duration::from_secs(86400))
                .unwrap()
                .files
                .len(),
            1
        );
    }
    #[test]
    #[ignore = "read-only current-user cleanup scan; never deletes files"]
    fn expanded_cleaners_find_actual_cache_without_deleting() {
        let local = local_app_data_dir().unwrap();
        let mut scan = scan_root(&local.join("Temp")).unwrap();
        extend_cleaners(&mut scan, &local);
        for category in scan.summary().categories {
            println!(
                "{}: {} files / {}",
                category.label,
                category.file_count,
                format_bytes(category.size_bytes)
            );
        }
        assert!(scan
            .files
            .iter()
            .all(|f| scan.roots.iter().any(|r| f.path.starts_with(r))));
        assert!(scan
            .files
            .iter()
            .all(
                |f| !["cookies", "history", "login data", "preferences"].contains(
                    &f.path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .to_lowercase()
                        .as_str()
                )
            ));
    }
    #[test]
    fn files_held_by_application_are_not_deleted() {
        let root = tempfile::tempdir().unwrap();
        let path = old_file(root.path(), "in-use.tmp");
        let _writer = fs::OpenOptions::new()
            .write(true)
            .share_mode(3)
            .open(&path)
            .unwrap();
        assert!(!clean_scan(scan_root(root.path()).unwrap(), &["temporary".into()]).success);
        assert!(path.exists());
    }
    #[test]
    fn only_selected_category_is_deleted() {
        let root = tempfile::tempdir().unwrap();
        let temp = old_file(root.path(), "old.tmp");
        let log = old_file(root.path(), "old.log");
        assert!(clean_scan(scan_root(root.path()).unwrap(), &["logs".into()]).success);
        assert!(temp.exists());
        assert!(!log.exists());
    }
    #[test]
    fn modified_after_scan_is_preserved() {
        let root = tempfile::tempdir().unwrap();
        let path = old_file(root.path(), "old.tmp");
        let scan = scan_root(root.path()).unwrap();
        fs::write(&path, "changed").unwrap();
        assert!(!clean_scan(scan, &["temporary".into()]).success);
        assert!(path.exists());
    }
    #[test]
    fn paths_outside_root_are_preserved() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let path = old_file(outside.path(), "keep.tmp");
        let mut scan = scan_root(root.path()).unwrap();
        let m = fs::metadata(&path).unwrap();
        scan.files.push(Candidate {
            path: path.clone(),
            category: "temporary",
            size: m.len(),
            modified: m.modified().unwrap(),
            min_age: MIN_AGE,
        });
        assert!(!clean_scan(scan, &["temporary".into()]).success);
        assert!(path.exists());
    }
    #[test]
    fn consumed_and_stale_scans_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let mut scan = scan_root(root.path()).unwrap();
        scan.created = Instant::now() - SCAN_TTL - Duration::from_secs(1);
        let id = scan.id.clone();
        let state = CleaningState(Mutex::new(Some(scan)));
        assert!(take_scan(&state, &id, &["temporary".into()]).is_err());
        let scan = scan_root(root.path()).unwrap();
        let id = scan.id.clone();
        *state.0.lock().unwrap() = Some(scan);
        assert!(take_scan(&state, &id, &["invalid".into()]).is_err());
        assert!(take_scan(&state, &id, &["temporary".into()]).is_ok());
        assert!(take_scan(&state, &id, &["temporary".into()]).is_err());
    }
    #[test]
    fn junctions_are_not_traversed() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let file = old_file(outside.path(), "keep.tmp");
        let link = root.path().join("linked");
        let status = std::process::Command::new("cmd.exe")
            .args(["/c", "mklink", "/J"])
            .arg(&link)
            .arg(outside.path())
            .creation_flags(0x08000000)
            .status()
            .unwrap();
        assert!(status.success());
        let scan = scan_root(root.path()).unwrap();
        assert!(scan.files.is_empty());
        assert!(scan.skipped > 0);
        clean_scan(scan, &["temporary".into()]);
        assert!(file.exists());
        fs::remove_dir(link).unwrap();
    }
}

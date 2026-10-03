use crate::{build_action_result, format_bytes, local_app_data_dir, ToolActionResult};
use serde::Serialize;
use std::{
    fs,
    os::windows::fs::MetadataExt,
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
}

pub(super) fn no_reparse_ancestors(path: &Path) -> bool {
    path.ancestors().all(|ancestor| {
        fs::symlink_metadata(ancestor)
            .map(|m| m.file_attributes() & REPARSE_POINT == 0)
            .unwrap_or(false)
    })
}
fn eligible(metadata: &fs::Metadata, now: SystemTime) -> bool {
    metadata.is_file()
        && metadata.file_attributes() & REPARSE_POINT == 0
        && metadata
            .modified()
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age >= MIN_AGE)
}
fn scan_root(root: &Path) -> Result<Scan, String> {
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
            Ok(metadata) if eligible(&metadata, now) => {
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
        }
    }
}
fn take_scan(state: &CleaningState, scan_id: &str, categories: &[String]) -> Result<Scan, String> {
    if categories.is_empty()
        || categories
            .iter()
            .any(|id| id != "temporary" && id != "logs" && id != "caches")
    {
        return Err("请选择有效的清理项目。".into());
    }
    let mut current = state.0.lock().map_err(|_| "清理任务状态异常".to_string())?;
    let scan = current.as_ref().ok_or("请先扫描临时文件。")?;
    if scan.id != scan_id || scan.created.elapsed() > SCAN_TTL {
        return Err("扫描结果已失效，请重新扫描。".into());
    }
    Ok(current.take().unwrap())
}
fn clean_scan(scan: Scan, categories: &[String]) -> ToolActionResult {
    let started = Instant::now();
    let mut freed = 0;
    let mut removed = 0;
    let mut skipped = 0;
    for file in scan
        .files
        .iter()
        .filter(|file| categories.iter().any(|id| id == file.category))
    {
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
        if !eligible(&metadata, SystemTime::now())
            || metadata.len() != file.size
            || metadata.modified().ok() != Some(file.modified)
        {
            skipped += 1;
            continue;
        }
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
            match scan_root(&path) {
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
        Ok::<_, String>(scan)
    })
    .await
    .map_err(|e| e.to_string())??;
    let summary = scan.summary();
    *state.0.lock().map_err(|_| "清理任务状态异常".to_string())? = Some(scan);
    Ok(summary)
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
    fn scan_excludes_recent_files() {
        let root = tempfile::tempdir().unwrap();
        old_file(root.path(), "old.tmp");
        fs::write(root.path().join("new.tmp"), "keep").unwrap();
        let scan = scan_root(root.path()).unwrap();
        assert_eq!(scan.files.len(), 1);
        assert_eq!(scan.skipped, 1);
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

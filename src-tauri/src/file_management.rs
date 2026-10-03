use crate::cleaning::no_reparse_ancestors;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::os::windows::fs::MetadataExt;
use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime},
};
use walkdir::WalkDir;
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Default)]
pub struct FileState {
    scan: Mutex<Vec<Scan>>,
    busy: AtomicBool,
    jobs: [ScanJob; 2],
}
#[derive(Default)]
struct ScanJob {
    busy: AtomicBool,
    cancel: Arc<AtomicBool>,
}
impl FileState {
    pub(crate) fn cancel_jobs(&self, duplicates: Option<bool>) {
        for (index, job) in self.jobs.iter().enumerate() {
            if duplicates.is_none_or(|mode| usize::from(mode) == index) {
                job.cancel.store(true, Ordering::SeqCst);
            }
        }
    }
}
struct BusyGuard<'a>(&'a AtomicBool);
impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
#[derive(Clone)]
struct Candidate {
    path: PathBuf,
    size: u64,
    modified: SystemTime,
    group: Option<usize>,
    hash: Option<Vec<u8>>,
}
struct Scan {
    id: String,
    root: PathBuf,
    created: Instant,
    files: Vec<Candidate>,
}
fn path_within(path: &Path, root: &Path) -> bool {
    PathBuf::from(
        path.to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_lowercase(),
    )
    .starts_with(PathBuf::from(
        root.to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_lowercase(),
    ))
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRow {
    pub id: usize,
    pub path: String,
    pub name: String,
    pub size_bytes: u64,
    pub group: Option<usize>,
    pub can_recycle: bool,
    pub modified_at: u64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageNode {
    path: String,
    name: String,
    size_bytes: u64,
    file_count: u64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileScan {
    pub scan_id: String,
    pub files: Vec<FileRow>,
    pub skipped: usize,
    pub limited: bool,
    usage: Vec<UsageNode>,
    pub total_bytes: u64,
    breakdown: Vec<StorageCategory>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageCategory {
    id: &'static str,
    label: &'static str,
    size_bytes: u64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveInfo {
    path: String,
    total_bytes: u64,
    free_bytes: u64,
}
#[tauri::command]
pub fn storage_drives() -> Vec<DriveInfo> {
    file_scan_drives()
        .into_iter()
        .filter_map(|path| {
            let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
            let (mut total, mut free) = (0u64, 0u64);
            unsafe {
                windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
                    windows::core::PCWSTR(wide.as_ptr()),
                    None,
                    Some(&mut total),
                    Some(&mut free),
                )
                .ok()?;
            }
            Some(DriveInfo {
                path,
                total_bytes: total,
                free_bytes: free,
            })
        })
        .collect()
}
fn storage_kind(path: &Path) -> &'static str {
    let parts: Vec<_> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect();
    if parts.iter().any(|s| s == "$recycle.bin") {
        "recycle"
    } else if parts.iter().any(|s| s == "windows")
        || path
            .parent()
            .is_some_and(|p| p.to_string_lossy().trim_end_matches('\\').ends_with(':'))
    {
        "system"
    } else if parts.iter().any(|s| {
        [
            "appdata",
            "programdata",
            "node_modules",
            ".git",
            "program files",
            "program files (x86)",
        ]
        .contains(&s.as_str())
    }) {
        "applications"
    } else if std::env::var_os("USERPROFILE")
        .is_some_and(|root| path_within(path, &PathBuf::from(root)))
        || parts.iter().any(|s| s == "users")
    {
        "personal"
    } else {
        "other"
    }
}

fn protected(path: &Path) -> bool {
    for key in [
        "SystemRoot",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramData",
        "LOCALAPPDATA",
        "APPDATA",
    ] {
        if let Some(root) = std::env::var_os(key) {
            if path_within(path, &PathBuf::from(root)) {
                return true;
            }
        }
    }
    path.components().any(|c| {
        [
            "$recycle.bin",
            "system volume information",
            "windows",
            "appdata",
            "program files",
            "program files (x86)",
            "programdata",
            "node_modules",
            ".git",
        ]
        .contains(&c.as_os_str().to_string_lossy().to_lowercase().as_str())
    }) || (!path.is_dir()
        && path
            .parent()
            .is_some_and(|p| p.to_string_lossy().trim_end_matches('\\').ends_with(':')))
}
#[tauri::command]
pub fn file_scan_drives() -> Vec<String> {
    ('A'..='Z')
        .map(|letter| format!("{letter}:\\"))
        .filter(|path| Path::new(path).is_dir())
        .collect()
}
fn unchanged(file: &Candidate, root: &Path) -> bool {
    file.path.starts_with(root)
        && no_reparse_ancestors(&file.path)
        && fs::symlink_metadata(&file.path).is_ok_and(|m| {
            m.is_file() && m.len() == file.size && m.modified().ok() == Some(file.modified)
        })
}
fn hash_file(path: &Path, cancel: &AtomicBool, started: Instant) -> Result<Vec<u8>, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut digest = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err("扫描已取消".into());
        }
        if started.elapsed() > Duration::from_secs(180) {
            return Err("扫描达到时间上限，请缩小目录范围".into());
        }
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        digest.update(&buf[..n]);
    }
    Ok(digest.finalize().to_vec())
}
#[cfg(test)]
fn scan_files(
    root: PathBuf,
    duplicates: bool,
    cancel: &AtomicBool,
) -> Result<(Scan, usize, bool, Vec<UsageNode>, u64, Vec<StorageCategory>), String> {
    scan_files_budget(root, duplicates, cancel, 180)
}
fn scan_files_budget(
    root: PathBuf,
    duplicates: bool,
    cancel: &AtomicBool,
    seconds: u64,
) -> Result<(Scan, usize, bool, Vec<UsageNode>, u64, Vec<StorageCategory>), String> {
    if !root.is_absolute() || !root.is_dir() || !no_reparse_ancestors(&root) {
        return Err("目录不存在或包含链接。".into());
    }
    let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
    let started = Instant::now();
    let mut files = Vec::new();
    let mut skipped = 0;
    let mut limited = false;
    let mut usage: BTreeMap<PathBuf, (u64, u64)> = BTreeMap::new();
    let mut total_bytes = 0u64;
    let mut breakdown: BTreeMap<&str, u64> = BTreeMap::new();
    let walker = WalkDir::new(&root)
        .follow_links(false)
        .sort_by_key(|entry| {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            let rank = if [
                "users",
                "downloads",
                "documents",
                "desktop",
                "pictures",
                "videos",
            ]
            .contains(&name.as_str())
            {
                0
            } else if [
                "windows",
                "program files",
                "program files (x86)",
                "programdata",
            ]
            .contains(&name.as_str())
            {
                2
            } else {
                1
            };
            (rank, name)
        })
        .into_iter()
        .filter_entry(|e| {
            !(duplicates && e.depth() > 0 && e.file_type().is_dir() && protected(e.path()))
                && fs::symlink_metadata(e.path()).is_ok_and(|m| m.file_attributes() & 0x400 == 0)
        });
    for (visited, entry) in walker.enumerate() {
        if cancel.load(Ordering::SeqCst) {
            return Err("扫描已取消".into());
        }
        if visited >= 200_000 || started.elapsed() > Duration::from_secs(seconds) {
            limited = true;
            break;
        }
        let entry = match entry {
            Ok(e) => e,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let m = match entry.metadata() {
            Ok(m) => m,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        if m.len() == 0 {
            continue;
        }
        total_bytes = total_bytes.saturating_add(m.len());
        let size = breakdown.entry(storage_kind(entry.path())).or_default();
        *size = size.saturating_add(m.len());
        if !duplicates {
            for parent in entry
                .path()
                .ancestors()
                .skip(1)
                .take_while(|p| p.starts_with(&root))
            {
                if usage.len() >= 50000 && !usage.contains_key(parent) {
                    limited = true;
                    break;
                }
                let value = usage.entry(parent.to_path_buf()).or_default();
                value.0 = value.0.saturating_add(m.len());
                value.1 += 1;
            }
        }
        if duplicates && protected(entry.path()) {
            continue;
        }
        if let Ok(modified) = m.modified() {
            files.push(Candidate {
                path: entry.path().to_owned(),
                size: m.len(),
                modified,
                group: None,
                hash: None,
            });
        }
    }
    if duplicates {
        let mut sizes: BTreeMap<u64, Vec<Candidate>> = BTreeMap::new();
        for f in files {
            sizes.entry(f.size).or_default().push(f);
        }
        files = Vec::new();
        let mut group = 0;
        let mut hash_limit = false;
        for same_size in sizes.into_values().rev().filter(|v| v.len() > 1) {
            let mut hashes: BTreeMap<Vec<u8>, Vec<Candidate>> = BTreeMap::new();
            for mut f in same_size {
                if !unchanged(&f, &root) {
                    skipped += 1;
                    continue;
                }
                match hash_file(&f.path, cancel, started) {
                    Ok(hash) => {
                        if unchanged(&f, &root) {
                            f.hash = Some(hash.clone());
                            hashes.entry(hash).or_default().push(f);
                        } else {
                            skipped += 1;
                        }
                    }
                    Err(e) => {
                        if cancel.load(Ordering::SeqCst) {
                            return Err(e);
                        }
                        if started.elapsed() > Duration::from_secs(180) {
                            limited = true;
                            hash_limit = true;
                            break;
                        }
                        skipped += 1;
                    }
                }
            }
            for mut values in hashes.into_values().filter(|v| v.len() > 1) {
                for f in &mut values {
                    f.group = Some(group);
                }
                group += 1;
                files.extend(values);
            }
            if hash_limit {
                break;
            }
        }
    } else {
        files.sort_by(|a, b| b.size.cmp(&a.size));
    }
    // Bound the displayed result and keep complete duplicate groups.
    if !duplicates && files.len() > 2000 {
        files.truncate(2000);
        limited = true;
    }
    if duplicates && files.len() > 5000 {
        files.truncate(5000);
        let mut counts = BTreeMap::new();
        for file in &files {
            *counts.entry(file.group).or_insert(0usize) += 1;
        }
        files.retain(|file| counts[&file.group] > 1);
        limited = true;
    }
    let mut usage: Vec<_> = usage
        .into_iter()
        .map(|(path, (size_bytes, file_count))| UsageNode {
            name: path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into(),
            path: path.to_string_lossy().into(),
            size_bytes,
            file_count,
        })
        .collect();
    usage.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
    if usage.len() > 10000 {
        usage.truncate(10000);
        limited = true;
    }
    Ok((
        Scan {
            id: format!(
                "files-{}-{}",
                crate::unix_timestamp_slug(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ),
            root,
            created: Instant::now(),
            files,
        },
        skipped,
        limited,
        usage,
        total_bytes,
        [
            ("personal", "用户文件"),
            ("applications", "应用文件"),
            ("system", "系统文件"),
            ("recycle", "回收站"),
            ("other", "其他"),
        ]
        .into_iter()
        .map(|(id, label)| StorageCategory {
            id,
            label,
            size_bytes: *breakdown.get(id).unwrap_or(&0),
        })
        .collect(),
    ))
}
#[tauri::command]
pub async fn scan_personal_files(
    root: String,
    duplicates: bool,
    state: tauri::State<'_, FileState>,
) -> Result<FileScan, String> {
    scan_for_state(root, duplicates, &state).await
}
pub(crate) async fn scan_for_state(
    root: String,
    duplicates: bool,
    state: &FileState,
) -> Result<FileScan, String> {
    scan_for_state_budget(root, duplicates, state, 180).await
}
pub(crate) async fn scan_for_state_budget(
    root: String,
    duplicates: bool,
    state: &FileState,
    seconds: u64,
) -> Result<FileScan, String> {
    let job = &state.jobs[usize::from(duplicates)];
    if job.busy.swap(true, Ordering::SeqCst) {
        return Err("扫描正在进行。".into());
    }
    let _guard = BusyGuard(&job.busy);
    job.cancel.store(false, Ordering::SeqCst);

    let cancel = job.cancel.clone();
    let (scan, skipped, limited, usage, total_bytes, breakdown) =
        tauri::async_runtime::spawn_blocking(move || {
            scan_files_budget(PathBuf::from(root), duplicates, &cancel, seconds)
        })
        .await
        .map_err(|e| e.to_string())??;
    let result = FileScan {
        scan_id: scan.id.clone(),
        skipped,
        limited,
        usage,
        total_bytes,
        breakdown,
        files: scan
            .files
            .iter()
            .enumerate()
            .map(|(id, f)| FileRow {
                id,
                path: f.path.to_string_lossy().into(),
                name: f
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into(),
                size_bytes: f.size,
                group: f.group,
                can_recycle: !protected(&f.path),
                modified_at: f
                    .modified
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            })
            .collect(),
    };
    let mut scans = state.scan.lock().map_err(|e| e.to_string())?;
    scans.retain(|item| item.created.elapsed() < Duration::from_secs(900));
    if scans.len() >= 32 {
        scans.remove(0);
    }
    scans.push(scan);
    Ok(result)
}
#[tauri::command]
pub fn cancel_file_scan(duplicates: Option<bool>, state: tauri::State<'_, FileState>) {
    state.cancel_jobs(duplicates);
}
fn validate_selection(scan: &Scan, ids: &[usize]) -> Result<Vec<Candidate>, String> {
    if scan.created.elapsed() > Duration::from_secs(900) {
        return Err("扫描结果已过期。".into());
    }
    if ids.is_empty() || ids.len() > 2000 || ids.iter().any(|i| *i >= scan.files.len()) {
        return Err("请选择有效文件。".into());
    }
    let selected: HashSet<_> = ids.iter().copied().collect();
    let mut result = Vec::new();
    for id in selected.iter() {
        let file = &scan.files[*id];
        if protected(&file.path) {
            return Err("系统与应用文件仅用于空间统计，不能在此删除。".into());
        }
        if !unchanged(file, &scan.root) {
            return Err("文件已变化，请重新扫描。".into());
        }
        if let Some(group) = file.group {
            let keeper = scan
                .files
                .iter()
                .enumerate()
                .find(|(idx, f)| f.group == Some(group) && !selected.contains(idx))
                .map(|(_, f)| f)
                .ok_or("每组重复文件必须至少保留一个。")?;
            if !unchanged(keeper, &scan.root) {
                return Err("保留文件已变化，请重新扫描。".into());
            }
            let cancel = AtomicBool::new(false);
            let started = Instant::now();
            if hash_file(&file.path, &cancel, started)? != file.hash.clone().unwrap()
                || hash_file(&keeper.path, &cancel, started)? != file.hash.clone().unwrap()
            {
                return Err("文件内容已变化，请重新扫描。".into());
            }
        }
        result.push(file.clone());
    }
    Ok(result)
}
#[tauri::command]
pub async fn recycle_selected_files(
    scan_id: String,
    ids: Vec<usize>,
    state: tauri::State<'_, FileState>,
) -> Result<String, String> {
    recycle_for_state(scan_id, ids, &state).await
}
pub(crate) async fn recycle_for_state(
    scan_id: String,
    ids: Vec<usize>,
    state: &FileState,
) -> Result<String, String> {
    recycle_for_state_result(scan_id, ids, state)
        .await
        .map(|(message, _)| message)
}
pub(crate) async fn recycle_for_state_result(
    scan_id: String,
    ids: Vec<usize>,
    state: &FileState,
) -> Result<(String, usize), String> {
    if state.busy.swap(true, Ordering::SeqCst) {
        return Err("任务正在进行。".into());
    }
    let _guard = BusyGuard(&state.busy);
    let scan = {
        let mut slot = state.scan.lock().map_err(|e| e.to_string())?;
        let index = slot
            .iter()
            .position(|s| s.id == scan_id)
            .ok_or("扫描结果已失效，请重新扫描。")?;
        validate_selection(&slot[index], &ids)?;
        slot.remove(index)
    };
    tauri::async_runtime::spawn_blocking(move || {
        let files = validate_selection(&scan, &ids)?;
        let mut count = 0;
        let mut failed = 0;
        for f in files {
            if !unchanged(&f, &scan.root) {
                failed += 1;
                continue;
            }
            match crate::recycle::recycle(&f.path) {
                Ok(()) => count += 1,
                Err(_) => failed += 1,
            }
        }
        Ok((
            format!("已移到回收站 {count} 个文件，未移动 {failed} 个。"),
            failed,
        ))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn personal_folders() -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(||{let raw=crate::run_powershell_json(r#"
$folders=@([Environment]::GetFolderPath('Desktop'),[Environment]::GetFolderPath('MyDocuments'))
$key=Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders'
$download=$key.'{374DE290-123F-4565-9164-39C4925E467B}';if($download){$folders += [Environment]::ExpandEnvironmentVariables($download)}
ConvertTo-Json -InputObject @($folders | Where-Object { $_ -and (Test-Path -LiteralPath $_) } | Select-Object -Unique) -Compress
"#)?;serde_json::from_str(&raw).map_err(|e|e.to_string())}).await.map_err(|e|e.to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scan_lanes_run_together_and_cancel_independently() {
        let state = FileState::default();
        assert!(!state.jobs[0].busy.swap(true, Ordering::SeqCst));
        let first = BusyGuard(&state.jobs[0].busy);
        assert!(!state.jobs[1].busy.swap(true, Ordering::SeqCst));
        let second = BusyGuard(&state.jobs[1].busy);
        assert!(state.jobs[0].busy.swap(true, Ordering::SeqCst));
        state.cancel_jobs(Some(true));
        assert!(!state.jobs[0].cancel.load(Ordering::SeqCst));
        assert!(state.jobs[1].cancel.load(Ordering::SeqCst));
        drop(second);
        assert!(state.jobs[0].busy.load(Ordering::SeqCst));
        assert!(!state.jobs[1].busy.load(Ordering::SeqCst));
        state.cancel_jobs(None);
        assert!(state.jobs[0].cancel.load(Ordering::SeqCst));
        drop(first);
        assert!(!state.jobs[0].busy.load(Ordering::SeqCst));
    }
    fn candidate(path: PathBuf, group: Option<usize>) -> Candidate {
        let m = fs::metadata(&path).unwrap();
        Candidate {
            path,
            size: m.len(),
            modified: m.modified().unwrap(),
            group,
            hash: None,
        }
    }
    #[test]
    #[ignore = "explicit read-only C drive scan and duplicate hashes; never deletes"]
    fn current_drive_scans_return_readable_files_and_duplicate_groups() {
        for duplicates in [false, true] {
            let started = Instant::now();
            let (scan, skipped, limited, _, bytes, _) =
                scan_files(PathBuf::from(r"C:\"), duplicates, &AtomicBool::new(false)).unwrap();
            println!("mode={duplicates} files={} groups={} bytes={bytes} skipped={skipped} limited={limited} elapsed={:?}",scan.files.len(),scan.files.iter().filter_map(|f|f.group).collect::<HashSet<_>>().len(),started.elapsed());
            assert!(
                !scan.files.is_empty(),
                "current test machine should have scan results"
            );
        }
    }
    #[test]
    fn multiple_scans_remain_independently_selectable() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sample");
        fs::write(&path, "sample").unwrap();
        let make = |id: &str| Scan {
            id: id.into(),
            root: directory.path().into(),
            created: Instant::now(),
            files: vec![candidate(path.clone(), None)],
        };
        let state = FileState::default();
        let mut scans = state.scan.lock().unwrap();
        scans.push(make("large"));
        scans.push(make("duplicates"));
        assert!(validate_selection(scans.iter().find(|s| s.id == "large").unwrap(), &[0]).is_ok());
        assert!(
            validate_selection(scans.iter().find(|s| s.id == "duplicates").unwrap(), &[0]).is_ok()
        );
    }
    #[test]
    fn protected_directory_checks_ignore_windows_case_and_respect_boundaries() {
        assert!(path_within(
            Path::new(r"C:\WINDOWS\System32"),
            Path::new(r"c:\Windows")
        ));
        assert!(!path_within(
            Path::new(r"C:\Windows-photos"),
            Path::new(r"c:\Windows")
        ));
    }
    #[test]
    fn top_level_user_directory_is_not_a_protected_root_file() {
        let users = Path::new(r"C:\Users");
        if users.is_dir() {
            assert!(!protected(users));
        }
        assert!(protected(Path::new(r"C:\pagefile.sys")));
    }
    #[test]
    fn system_files_remain_readonly_and_extended_paths_are_normalized() {
        assert!(protected(Path::new(r"\\?\C:\Windows\System32\a.dll")));
        assert!(protected(Path::new(r"D:\Program Files\App\a.exe")));
        assert!(protected(Path::new(r"\\?\C:\pagefile.sys")));
        assert!(!protected(Path::new(r"D:\Photos\image.jpg")));
    }
    #[test]
    fn space_scan_counts_small_files_and_nested_folder_totals() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("nested")).unwrap();
        fs::write(root.path().join("a"), [0u8; 10]).unwrap();
        fs::write(root.path().join("nested/b"), [0u8; 30]).unwrap();
        let (scan, _, _, usage, total, breakdown) =
            scan_files(root.path().into(), false, &AtomicBool::new(false)).unwrap();
        assert_eq!(total, 40);
        assert_eq!(breakdown.iter().map(|v| v.size_bytes).sum::<u64>(), total);
        assert_eq!(scan.files.len(), 2);
        assert!(usage
            .iter()
            .any(|n| n.name == "nested" && n.size_bytes == 30));
        assert_eq!(usage[0].size_bytes, 40);
    }
    #[test]
    fn changed_or_outside_file_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a");
        fs::write(&p, "hello").unwrap();
        let f = candidate(p.clone(), None);
        assert!(unchanged(&f, dir.path()));
        fs::write(&p, "changed").unwrap();
        assert!(!unchanged(&f, dir.path()));
        assert!(!unchanged(&f, Path::new(r"C:\not-the-root")));
    }
    #[test]
    fn duplicate_group_cannot_be_deleted_entirely() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a");
        let q = dir.path().join("b");
        fs::write(&p, "same").unwrap();
        fs::write(&q, "same").unwrap();
        let s = Scan {
            id: "test".into(),
            root: dir.path().to_owned(),
            created: Instant::now(),
            files: vec![candidate(p, Some(0)), candidate(q, Some(0))],
        };
        assert!(validate_selection(&s, &[0, 1]).is_err());
    }
    #[test]
    fn same_size_different_content_is_not_duplicate() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a");
        let q = dir.path().join("b");
        fs::write(&p, "aaaa").unwrap();
        fs::write(&q, "bbbb").unwrap();
        let c = AtomicBool::new(false);
        assert_ne!(
            hash_file(&p, &c, Instant::now()).unwrap(),
            hash_file(&q, &c, Instant::now()).unwrap()
        );
    }
}

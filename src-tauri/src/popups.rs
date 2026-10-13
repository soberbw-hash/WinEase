use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock,
    },
    time::{Duration, Instant},
};
use windows::core::BOOL;
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::{Foundation::*, System::Threading::*, UI::WindowsAndMessaging::*};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    id: String,
    path: String,
    title: String,
}
#[derive(Default)]
struct Rules {
    items: Vec<Rule>,
    load_error: Option<String>,
    requests: u64,
}
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
static RULES: OnceLock<Mutex<Rules>> = OnceLock::new();
fn location() -> Result<PathBuf, String> {
    Ok(crate::local_app_data_dir()
        .ok_or("无法定位弹窗规则目录")?
        .join("WinToolbox")
        .join("popup-rules.json"))
}
fn state() -> &'static Mutex<Rules> {
    RULES.get_or_init(|| {
        let mut result = Rules::default();
        if let Ok(path) = location() {
            if path.exists() {
                if !crate::cleaning::no_reparse_ancestors(&path) {
                    result.load_error = Some("弹窗规则包含链接，已停止读取。".into());
                } else {
                    match fs::read(&path)
                        .map_err(|e| e.to_string())
                        .and_then(|bytes| {
                            serde_json::from_slice::<Vec<Rule>>(&bytes).map_err(|e| e.to_string())
                        }) {
                        Ok(items) => result.items = items,
                        Err(e) => result.load_error = Some(format!("弹窗规则无法读取：{e}")),
                    }
                }
            }
        }
        Mutex::new(result)
    })
}
fn save(items: &[Rule]) -> Result<(), String> {
    let path = location()?;
    let dir = path.parent().unwrap();
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    if !crate::cleaning::no_reparse_ancestors(dir)
        || (path.exists() && !crate::cleaning::no_reparse_ancestors(&path))
    {
        return Err("规则目录包含链接，已停止保存".into());
    }
    use std::io::Write;
    let temp = dir.join(format!(
        "popup-rules-{}-{}.tmp",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        file.write_all(&serde_json::to_vec_pretty(items).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        fs::rename(&temp, &path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}
fn matches_rule(rule: &Rule, path: &str, title: &str) -> bool {
    rule.path.eq_ignore_ascii_case(path) && rule.title == title && !title.is_empty()
}
#[tauri::command]
pub fn list_popup_rules() -> Result<serde_json::Value, String> {
    let rules = state().lock().map_err(|e| e.to_string())?;
    if let Some(error) = &rules.load_error {
        return Err(error.clone());
    }
    Ok(serde_json::json!({"rules":rules.items,"requests":rules.requests}))
}
#[tauri::command]
pub async fn add_popup_rule(pid: u32, stamp: String, window_id: String) -> Result<(), String> {
    let rows = crate::management::list_application_windows().await?;
    let item = rows
        .as_array()
        .and_then(|items| {
            items.iter().find(|i| {
                i["pid"].as_u64() == Some(pid as u64)
                    && i["stamp"].as_str() == Some(stamp.as_str())
                    && i["windowId"].as_str() == Some(window_id.as_str())
            })
        })
        .ok_or("窗口已变化，请刷新列表")?;
    if item["canEnd"].as_bool() != Some(true) {
        return Err("不能拦截受保护的窗口".into());
    }
    let path = item["path"].as_str().ok_or("无法读取应用路径")?.to_owned();
    let title = item["title"].as_str().ok_or("无法读取窗口标题")?.to_owned();
    let mut rules = state().lock().map_err(|e| e.to_string())?;
    if let Some(error) = &rules.load_error {
        return Err(error.clone());
    }
    if rules.items.iter().any(|r| matches_rule(r, &path, &title)) {
        return Ok(());
    }
    if rules.items.len() >= 50 {
        return Err("最多可保存 50 条拦截规则。".into());
    }
    let mut items = rules.items.clone();
    items.push(Rule {
        id: format!(
            "popup-{}-{}",
            crate::unix_timestamp_slug(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ),
        path,
        title,
    });
    save(&items)?;
    rules.items = items;
    Ok(())
}
#[tauri::command]
pub fn remove_popup_rule(id: String) -> Result<(), String> {
    let mut rules = state().lock().map_err(|e| e.to_string())?;
    if let Some(error) = &rules.load_error {
        return Err(error.clone());
    }
    let items = rules
        .items
        .iter()
        .filter(|r| r.id != id)
        .cloned()
        .collect::<Vec<_>>();
    save(&items)?;
    rules.items = items;
    Ok(())
}

struct Scan {
    rules: Vec<Rule>,
    hits: Vec<HWND>,
    session: u32,
}
unsafe extern "system" fn visit(hwnd: HWND, param: LPARAM) -> BOOL {
    let scan = &mut *(param.0 as *mut Scan);
    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }
    let mut text = [0u16; 1024];
    let n = GetWindowTextW(hwnd, &mut text);
    if n <= 0 {
        return BOOL(1);
    }
    let title = String::from_utf16_lossy(&text[..n as usize]);
    if !scan.rules.iter().any(|r| r.title == title) {
        return BOOL(1);
    }
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    let mut session = 0;
    if pid == std::process::id()
        || pid <= 4
        || ProcessIdToSessionId(pid, &mut session).is_err()
        || session != scan.session
    {
        return BOOL(1);
    }
    if let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
        let mut path = [0u16; 32768];
        let mut len = path.len() as u32;
        if QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(path.as_mut_ptr()),
            &mut len,
        )
        .is_ok()
        {
            let path = String::from_utf16_lossy(&path[..len as usize]);
            let protected = std::env::var("WINDIR")
                .is_ok_and(|dir| path.to_lowercase().starts_with(&dir.to_lowercase()));
            if !protected && scan.rules.iter().any(|r| matches_rule(r, &path, &title)) {
                scan.hits.push(hwnd);
            }
        }
        let _ = CloseHandle(handle);
    }
    BOOL(1)
}
pub fn start_worker() {
    std::thread::spawn(|| {
        let mut recent = std::collections::HashMap::<isize, Instant>::new();
        loop {
            std::thread::sleep(Duration::from_secs(2));
            let items = match state().lock() {
                Ok(r) if r.load_error.is_none() => r.items.clone(),
                _ => continue,
            };
            if items.is_empty() {
                continue;
            }
            let mut session = 0;
            unsafe {
                let _ = ProcessIdToSessionId(std::process::id(), &mut session);
            }
            let mut scan = Scan {
                rules: items,
                hits: Vec::new(),
                session,
            };
            unsafe {
                let _ = EnumWindows(Some(visit), LPARAM(&mut scan as *mut Scan as isize));
            }
            recent.retain(|_, time| time.elapsed() < Duration::from_secs(30));
            for hwnd in scan.hits {
                let key = hwnd.0 as isize;
                if recent.contains_key(&key) {
                    continue;
                }
                let mut result = 0;
                unsafe {
                    if SendMessageTimeoutW(
                        hwnd,
                        WM_CLOSE,
                        WPARAM(0),
                        LPARAM(0),
                        SMTO_ABORTIFHUNG,
                        200,
                        Some(&mut result),
                    )
                    .0 != 0
                    {
                        if let Ok(mut r) = state().lock() {
                            r.requests += 1;
                        }
                    }
                }
                recent.insert(key, Instant::now());
            }
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rules_require_both_exact_title_and_application() {
        let r = Rule {
            id: "test".into(),
            path: r"C:\Apps\a.exe".into(),
            title: "广告窗口".into(),
        };
        assert!(matches_rule(&r, r"c:\apps\A.exe", "广告窗口"));
        assert!(!matches_rule(&r, r"C:\Apps\b.exe", "广告窗口"));
        assert!(!matches_rule(&r, r"C:\Apps\a.exe", "主窗口"));
    }
}

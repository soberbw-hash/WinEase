//! One reviewed plan owns its evidence, cache/file tokens and executable actions.
//! The renderer sends only IDs from this plan; it cannot supply paths or commands.
use crate::{cleaning, file_management, health, management, network};
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime},
};

const TTL: Duration = Duration::from_secs(600);
#[derive(Default)]
pub struct OptimizationState(pub Arc<Core>);
#[derive(Default)]
pub struct Core {
    session: Mutex<Option<Session>>,
}
struct Session {
    report: Report,
    created: Instant,
    actions: BTreeMap<String, Action>,
    resources: Arc<Resources>,
}
#[derive(Default)]
struct Resources {
    cleaning: cleaning::CleaningState,
    files: file_management::FileState,
}
#[derive(Clone)]
enum Action {
    Clean {
        scan_id: String,
        groups: Vec<String>,
    },
    Recycle {
        scan_id: String,
        file: usize,
    },
    Startup {
        name: String,
        command: String,
    },
    Network {
        scan_id: String,
    },
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub status: String,
    pub size_bytes: u64,
    pub file_count: u64,
    pub selectable: bool,
    pub default_selected: bool,
    pub action_label: Option<String>,
    pub target: Option<String>,
    pub icon_target: Option<String>,
    pub icon_kind: String,
    pub has_files: bool,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub id: String,
    pub title: String,
    pub status: String,
    pub summary: String,
    pub items: Vec<Item>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub item_ids: Vec<String>,
    pub title: String,
    pub status: String,
    pub message: String,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub id: String,
    pub revision: u64,
    pub phase: String,
    pub checked_at: String,
    pub sections: Vec<Section>,
    pub outcomes: Vec<Outcome>,
    pub completed_actions: usize,
    pub total_actions: usize,
}
struct Findings {
    summary: String,
    items: Vec<Item>,
    actions: BTreeMap<String, Action>,
}
fn item(
    id: impl Into<String>,
    title: impl Into<String>,
    detail: impl Into<String>,
    status: &str,
) -> Item {
    Item {
        id: id.into(),
        title: title.into(),
        detail: detail.into(),
        status: status.into(),
        size_bytes: 0,
        file_count: 0,
        selectable: false,
        default_selected: false,
        action_label: None,
        target: None,
        icon_target: None,
        icon_kind: "system".into(),
        has_files: false,
    }
}
fn enable(row: &mut Item, label: &str, recommended: bool) {
    row.selectable = true;
    row.default_selected = recommended;
    row.action_label = Some(label.into());
}
fn blank_report() -> Report {
    Report {
        id: uuid::Uuid::new_v4().to_string(),
        revision: 0,
        phase: "checking".into(),
        checked_at: chrono::Utc::now().to_rfc3339(),
        sections: [
            ("cleaning", "深度清理"),
            ("files", "大文件与空间"),
            ("health", "电脑体检"),
            ("network", "网络检测"),
            ("startup", "开机管理"),
        ]
        .into_iter()
        .map(|(id, title)| Section {
            id: id.into(),
            title: title.into(),
            status: "checking".into(),
            summary: "检查中…".into(),
            items: vec![],
        })
        .collect(),
        outcomes: vec![],
        completed_actions: 0,
        total_actions: 0,
    }
}
fn publish(core: &Core, report_id: &str, section_id: &str, result: Result<Findings, String>) {
    let Ok(mut slot) = core.session.lock() else {
        return;
    };
    let Some(session) = slot
        .as_mut()
        .filter(|s| s.report.id == report_id && s.report.phase == "checking")
    else {
        return;
    };
    let Some(section) = session
        .report
        .sections
        .iter_mut()
        .find(|s| s.id == section_id)
    else {
        return;
    };
    match result {
        Ok(found) => {
            section.status = "complete".into();
            section.summary = found.summary;
            section.items = found.items;
            session.actions.extend(found.actions);
        }
        Err(error) => {
            section.status = "error".into();
            section.summary = format!("未完成：{error}");
        }
    }
    session.report.revision += 1;
}
async fn run_check(core: Arc<Core>, id: String, resources: Arc<Resources>) {
    let mut tasks = Vec::new();
    macro_rules! job {
        ($section:literal, $future:expr) => {{
            let core = core.clone();
            let id = id.clone();
            tasks.push((
                $section,
                tauri::async_runtime::spawn(async move {
                    publish(&core, &id, $section, $future.await);
                }),
            ));
        }};
    }
    {
        let resources = resources.clone();
        job!("cleaning", scan_cleaning(resources));
    }
    {
        let resources = resources.clone();
        job!("files", scan_files(resources));
    }
    job!("health", scan_health());
    job!("network", scan_network());
    job!("startup", scan_startup());
    for (section, task) in tasks {
        if let Err(error) = task.await {
            publish(&core, &id, section, Err(format!("检查任务异常：{error}")));
        }
    }
    if let Ok(mut slot) = core.session.lock() {
        if let Some(session) = slot
            .as_mut()
            .filter(|s| s.report.id == id && s.report.phase == "checking")
        {
            deduplicate_cache_files(session);
            session.report.phase = "ready".into();
            session.report.revision += 1;
        }
    }
}
fn deduplicate_cache_files(session: &mut Session) {
    let paths: HashSet<_> = cleaning::candidate_paths(&session.resources.cleaning)
        .into_iter()
        .collect();
    if let Some(section) = session
        .report
        .sections
        .iter_mut()
        .find(|section| section.id == "files")
    {
        for row in &mut section.items {
            if row.icon_target.as_ref().is_some_and(|path| {
                paths.contains(&path.trim_start_matches(r"\\?\").to_lowercase())
            }) {
                session.actions.remove(&row.id);
                row.selectable = false;
                row.default_selected = false;
                row.action_label = None;
                row.detail = format!(
                    "已归入深度清理，避免重复处理。\n{}",
                    row.icon_target
                        .as_deref()
                        .unwrap_or_default()
                        .trim_start_matches(r"\\?\")
                );
                row.status = "info".into();
            }
        }
    }
}
#[tauri::command]
pub fn start_optimization_check(
    state: tauri::State<'_, OptimizationState>,
) -> Result<Report, String> {
    start_check(state.0.clone())
}
fn start_check(core: Arc<Core>) -> Result<Report, String> {
    let resources = Arc::new(Resources::default());
    let report = blank_report();
    {
        let mut slot = core.session.lock().map_err(|_| "检查状态异常")?;
        if slot
            .as_ref()
            .is_some_and(|s| matches!(s.report.phase.as_str(), "checking" | "optimizing"))
        {
            return Err("当前检查或优化尚未结束".into());
        }
        *slot = Some(Session {
            report: report.clone(),
            created: Instant::now(),
            actions: BTreeMap::new(),
            resources: resources.clone(),
        });
    }
    tauri::async_runtime::spawn(run_check(core, report.id.clone(), resources));
    Ok(report)
}
#[tauri::command]
pub fn optimization_status(
    report_id: String,
    revision: Option<u64>,
    state: tauri::State<'_, OptimizationState>,
) -> Result<Option<Report>, String> {
    status(&state.0, &report_id, revision)
}
fn status(core: &Core, id: &str, revision: Option<u64>) -> Result<Option<Report>, String> {
    let mut slot = core.session.lock().map_err(|_| "检查状态异常")?;
    let session = slot
        .as_mut()
        .filter(|s| s.report.id == id)
        .ok_or("检查结果已被替换，请重新检查")?;
    if session.created.elapsed() > TTL && session.report.phase == "ready" {
        session.report.phase = "expired".into();
        session.report.revision += 1;
    }
    Ok((revision != Some(session.report.revision)).then(|| session.report.clone()))
}
async fn scan_cleaning(resources: Arc<Resources>) -> Result<Findings, String> {
    let scan = cleaning::scan_for_state(&resources.cleaning).await?;
    let mut grouped: BTreeMap<(String, bool), Vec<cleaning::CleaningGroup>> = BTreeMap::new();
    for group in scan.groups {
        grouped
            .entry((group.label.clone(), group.recommended))
            .or_default()
            .push(group);
    }
    let mut items = Vec::new();
    let mut actions = BTreeMap::new();
    for ((label, recommended), groups) in grouped {
        let id = format!("clean-{}", items.len());
        let mut row = item(
            &id,
            label,
            match groups[0].category {
                "downloads-cache" => "下载与构建缓存，清理后可能需要重新下载。",
                "caches" => "图形或缩略图缓存，清理后会重新生成。",
                "crashes" => "旧错误报告，清理后不再保留这些排查记录。",
                "temporary" | "logs" => "至少 7 天未修改；清理时再次核实文件是否变化或被占用。",
                _ => "至少 24 小时未修改的缓存；保留 Cookie、登录信息和个人文件。",
            },
            if recommended {
                "recommended"
            } else {
                "optional"
            },
        );
        row.size_bytes = groups.iter().map(|g| g.size_bytes).sum();
        row.file_count = groups.iter().map(|g| g.file_count).sum();
        row.icon_target = groups.iter().find_map(|g| g.icon_target.clone());
        row.icon_kind = "application".into();
        row.has_files = true;
        enable(&mut row, "清理缓存", recommended);
        actions.insert(
            id,
            Action::Clean {
                scan_id: scan.scan_id.clone(),
                groups: groups.into_iter().map(|g| g.id).collect(),
            },
        );
        items.push(row);
    }
    items.sort_by_key(|r| {
        (
            std::cmp::Reverse(r.default_selected),
            std::cmp::Reverse(r.size_bytes),
        )
    });
    let bytes: u64 = items.iter().map(|r| r.size_bytes).sum();
    let mut summary = if items.is_empty() {
        "未发现符合保留期限的清理项".to_string()
    } else {
        format!("发现 {} 可清理缓存", crate::format_bytes(bytes))
    };
    if scan.limited {
        summary.push_str("；达到扫描上限，当前为部分清理项，可处理后重新扫描。");
    }
    Ok(Findings {
        summary,
        items,
        actions,
    })
}
fn file_suggestion(
    file: &file_management::FileRow,
    downloads: Option<&std::path::Path>,
    now: u64,
) -> String {
    let path = std::path::PathBuf::from(&file.path);
    let old = now.saturating_sub(file.modified_at) >= 30 * 86400;
    let in_downloads = downloads.is_some_and(|folder| {
        path.to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_lowercase()
            .starts_with(&(folder.to_string_lossy().to_lowercase() + "\\"))
    });
    let extension = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !file.can_recycle {
        "系统或应用文件；请通过卸载程序或系统存储设置处理。".into()
    } else if old
        && in_downloads
        && ["exe", "msi", "zip", "7z", "rar"].contains(&extension.as_str())
    {
        "下载目录中超过 30 天未修改的安装包或压缩包；确认不再需要后可移到回收站。".into()
    } else {
        "仅因体积较大列出，不代表垃圾；确认用途后再处理。".into()
    }
}
async fn scan_files(resources: Arc<Resources>) -> Result<Findings, String> {
    let drives = file_management::file_scan_drives();
    if drives.is_empty() {
        return Err("未找到本地固定磁盘".into());
    }
    let mut items = Vec::new();
    let mut actions = BTreeMap::new();
    let mut limits = Vec::new();
    let mut failed = Vec::new();
    let downloads = crate::user_profile_dir().map(|p| p.join("Downloads"));
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut total = 0u64;
    let started = Instant::now();
    for drive in drives {
        let remaining = 180u64.saturating_sub(started.elapsed().as_secs());
        if remaining == 0 {
            failed.push(format!(
                "{drive}：本次扫描达到时间上限，可在空间管理中继续扫描"
            ));
            continue;
        }
        match file_management::scan_for_state_budget(
            drive.clone(),
            false,
            &resources.files,
            remaining.min(90),
        )
        .await
        {
            Ok(scan) => {
                total = total.saturating_add(scan.total_bytes);
                if scan.limited || scan.skipped > 0 {
                    limits.push(format!(
                        "{drive} {}{}",
                        if scan.limited {
                            "达到扫描/结果上限；"
                        } else {
                            ""
                        },
                        format!("跳过 {} 个不可读项目", scan.skipped)
                    ));
                }
                for file in scan
                    .files
                    .into_iter()
                    .filter(|f| f.size_bytes >= 100 * 1024 * 1024)
                {
                    let id = format!("file-{}-{}", scan.scan_id, file.id);
                    let mut row = item(
                        &id,
                        &file.name,
                        format!(
                            "{}\n{}",
                            file_suggestion(&file, downloads.as_deref(), now),
                            file.path.trim_start_matches(r"\\?\")
                        ),
                        "optional",
                    );
                    row.size_bytes = file.size_bytes;
                    row.file_count = 1;
                    row.icon_target = Some(file.path.clone());
                    row.icon_kind = "file".into();
                    if file.can_recycle {
                        enable(&mut row, "移到回收站", false);
                        actions.insert(
                            id,
                            Action::Recycle {
                                scan_id: scan.scan_id.clone(),
                                file: file.id,
                            },
                        );
                    }
                    row.target = std::path::Path::new(&file.path)
                        .parent()
                        .map(|p| p.to_string_lossy().trim_start_matches(r"\\?\").to_string());
                    items.push(row);
                }
            }
            Err(error) => failed.push(format!("{drive}：{error}")),
        }
    }
    items.sort_by_key(|r| std::cmp::Reverse(r.size_bytes));
    let mut summary = format!(
        "已统计 {}，列出 {} 个 ≥100 MB 文件",
        crate::format_bytes(total),
        items.len()
    );
    if !limits.is_empty() {
        summary.push_str(&format!("；部分统计：{}", limits.join("；")));
    }
    if !failed.is_empty() {
        summary.push_str(&format!("；未完成：{}", failed.join("；")));
    }
    if total == 0 && !failed.is_empty() {
        return Err(summary);
    }
    Ok(Findings {
        summary,
        items,
        actions,
    })
}
async fn scan_health() -> Result<Findings, String> {
    let report = health::check().await?;
    let items = report
        .checks
        .into_iter()
        .map(|check| {
            let mut row = item(
                format!("health-{}", check.id),
                check.title,
                check.detail,
                &check.status,
            );
            row.target = check.target;
            row
        })
        .collect::<Vec<_>>();
    let attention = items.iter().filter(|r| r.status == "attention").count();
    let unknown = items.iter().filter(|r| r.status == "unknown").count();
    Ok(Findings {
        summary: format!(
            "检查 {} 项，{attention} 项需关注{}",
            items.len(),
            if unknown > 0 {
                format!("，{unknown} 项未能核实")
            } else {
                String::new()
            }
        ),
        items,
        actions: BTreeMap::new(),
    })
}
async fn scan_network() -> Result<Findings, String> {
    let scan = network::network_scan().await?;
    let mut items = Vec::new();
    let mut actions = BTreeMap::new();
    if !scan.plan.is_empty() {
        let mut row = item(
            "network-fix",
            "修复检测到的网络配置问题",
            scan.plan
                .iter()
                .map(|tool| format!("{}：{}", tool.title, tool.description))
                .collect::<Vec<_>>()
                .join("\n"),
            "recommended",
        );
        let mild = scan
            .plan
            .iter()
            .all(|tool| !tool.disruptive && !tool.restart);
        if !mild {
            row.status = "optional".into();
            row.detail
                .push_str("\n可能短暂断网或需要重启电脑，执行前会备份配置。");
        }
        enable(&mut row, "备份并修复", mild);
        actions.insert(
            row.id.clone(),
            Action::Network {
                scan_id: scan.id.clone(),
            },
        );
        items.push(row);
    }
    for issue in &scan.issues {
        let informational = [
            "proxy-route",
            "proxy-programs",
            "managed",
            "site-restriction",
        ]
        .contains(&issue.id.as_str());
        let mut row = item(
            format!("network-{}", issue.id),
            &issue.title,
            &issue.detail,
            if informational {
                "info"
            } else if scan.state == "unknown" {
                "unknown"
            } else {
                "attention"
            },
        );
        row.target = Some("open_network".into());
        items.push(row);
    }
    for probe in &scan.probes {
        let mut row = item(
            format!("probe-{}", probe.id),
            &probe.name,
            format!(
                "{}{}",
                probe.detail,
                if probe.ok {
                    format!(" · {} ms", probe.latency_ms)
                } else {
                    String::new()
                }
            ),
            if probe.ok { "healthy" } else { "info" },
        );
        row.icon_kind = "network".into();
        items.push(row);
    }
    Ok(Findings {
        summary: scan.summary,
        items,
        actions,
    })
}
fn startup_suggestion(
    _name: &str,
    command: &str,
    enabled: bool,
    editable: bool,
) -> (&'static str, bool, bool) {
    if !enabled {
        return ("已关闭，无需优化。", false, false);
    }
    if !editable {
        return ("系统或所有用户启动项；请在开机管理中查看。", false, false);
    }
    // Match exact executable names, never substrings such as 'steam' anywhere in a path.
    let executable = if let Some(tail) = command.trim().strip_prefix('"') {
        tail.split('"').next().unwrap_or("")
    } else {
        command.split_whitespace().next().unwrap_or("")
    };
    let file = std::path::Path::new(executable)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match file.as_str() {
        "steam.exe" | "epicgameslauncher.exe" | "wegame.exe" => (
            "可关闭开机启动；需要时仍可手动打开，不会卸载软件。",
            true,
            true,
        ),
        "discord.exe" | "doubao.exe" | "qq.exe" | "weixin.exe" | "wechat.exe" => (
            "可选：关闭后不会随开机启动，可能错过消息通知。",
            true,
            false,
        ),
        _ => (
            "保留启动；网络、同步、驱动及未知程序不自动停用。",
            false,
            false,
        ),
    }
}
async fn scan_startup() -> Result<Findings, String> {
    let rows = management::list_startup_items().await?;
    let mut items = Vec::new();
    let mut actions = BTreeMap::new();
    let mut enabled_count = 0;
    for (index, entry) in rows.as_array().ok_or("启动项数据无效")?.iter().enumerate() {
        let name = entry["name"].as_str().unwrap_or("未命名启动项");
        let command = entry["command"].as_str().unwrap_or("");
        let enabled = entry["enabled"] == true;
        let editable = entry["editable"] == true;
        if enabled {
            enabled_count += 1;
        }
        let (reason, selectable, recommended) =
            startup_suggestion(name, command, enabled, editable);
        let mut row = item(
            format!("startup-{index}"),
            name,
            format!(
                "{} · {}\n{reason}",
                if enabled { "已开启" } else { "已关闭" },
                entry["source"].as_str().unwrap_or("启动项")
            ),
            if recommended {
                "recommended"
            } else if selectable {
                "optional"
            } else {
                "info"
            },
        );
        row.icon_target = Some(command.into());
        row.icon_kind = "command".into();
        row.target = Some("open_startup".into());
        if selectable {
            enable(&mut row, "关闭开机启动", recommended);
            actions.insert(
                row.id.clone(),
                Action::Startup {
                    name: name.into(),
                    command: command.into(),
                },
            );
        }
        items.push(row);
    }
    Ok(Findings {
        summary: format!(
            "{} 个启动项，{enabled_count} 个开启；仅建议已识别的非必要应用",
            items.len()
        ),
        items,
        actions,
    })
}
fn approved_actions(
    session: &Session,
    selected: &[String],
    confirmed: bool,
) -> Result<Vec<(String, Action)>, String> {
    if !confirmed {
        return Err("请先确认优化项目".into());
    }
    if session.report.phase != "ready" || session.created.elapsed() > TTL {
        return Err("请先完成检查；结果有效期为 10 分钟，过期后需重新检查".into());
    }
    let distinct: HashSet<_> = selected.iter().collect();
    if selected.is_empty() || selected.len() > 2000 || distinct.len() != selected.len() {
        return Err("请选择有效且不重复的优化项目（最多 2000 项）".into());
    }
    selected
        .iter()
        .map(|id| {
            session
                .actions
                .get(id)
                .cloned()
                .map(|action| (id.clone(), action))
                .ok_or("优化项目无效或仅供查看".into())
        })
        .collect()
}
#[tauri::command]
pub fn start_optimization(
    report_id: String,
    item_ids: Vec<String>,
    confirmed: bool,
    state: tauri::State<'_, OptimizationState>,
) -> Result<Report, String> {
    start_apply(state.0.clone(), report_id, item_ids, confirmed)
}
fn start_apply(
    core: Arc<Core>,
    report_id: String,
    item_ids: Vec<String>,
    confirmed: bool,
) -> Result<Report, String> {
    let (actions, resources, report) = {
        let mut slot = core.session.lock().map_err(|_| "检查状态异常")?;
        let session = slot
            .as_mut()
            .filter(|s| s.report.id == report_id)
            .ok_or("检查结果已被替换")?;
        let actions = approved_actions(session, &item_ids, confirmed)?;
        session.report.phase = "optimizing".into();
        session.report.total_actions = actions.len();
        session.report.revision += 1;
        (actions, session.resources.clone(), session.report.clone())
    };
    tauri::async_runtime::spawn(async move {
        apply_actions(&core, &report_id, &resources, actions).await;
        if let Ok(mut slot) = core.session.lock() {
            if let Some(s) = slot.as_mut().filter(|s| s.report.id == report_id) {
                s.report.phase = "complete".into();
                s.report.revision += 1;
            }
        }
    });
    Ok(report)
}
fn outcome(core: &Core, id: &str, items: Vec<String>, title: &str, result: Result<String, String>) {
    if let Ok(mut slot) = core.session.lock() {
        if let Some(s) = slot
            .as_mut()
            .filter(|s| s.report.id == id && s.report.phase == "optimizing")
        {
            let (status, message) = match result {
                Ok(message) => ("success", message),
                Err(error) => ("failed", error),
            };
            s.report.completed_actions += items.len();
            s.report.outcomes.push(Outcome {
                item_ids: items,
                title: title.into(),
                status: status.into(),
                message,
            });
            s.report.revision += 1;
        }
    }
}
async fn apply_actions(
    core: &Core,
    id: &str,
    resources: &Resources,
    actions: Vec<(String, Action)>,
) {
    // Collect cleanup groups and recycle IDs first: each scanner token is consumed once.
    let mut cleaning: BTreeMap<String, (Vec<String>, Vec<String>)> = BTreeMap::new();
    let mut recycle: BTreeMap<String, (Vec<String>, Vec<usize>)> = BTreeMap::new();
    let mut startup = Vec::new();
    let mut networks = Vec::new();
    for (item, action) in actions {
        match action {
            Action::Clean { scan_id, groups } => {
                let entry = cleaning.entry(scan_id).or_default();
                entry.0.push(item);
                entry.1.extend(groups);
            }
            Action::Recycle { scan_id, file } => {
                let entry = recycle.entry(scan_id).or_default();
                entry.0.push(item);
                entry.1.push(file);
            }
            Action::Startup { name, command } => startup.push((item, name, command)),
            Action::Network { scan_id } => networks.push((item, scan_id)),
        }
    }
    for (scan_id, (items, groups)) in cleaning {
        let result = cleaning::clean_for_state(&resources.cleaning, scan_id, groups)
            .await
            .and_then(|result| {
                if result.success {
                    Ok(result.summary)
                } else {
                    Err(format!("{} {}", result.summary, result.warnings.join("；")))
                }
            });
        outcome(core, id, items, "深度清理", result);
    }
    for (scan_id, (items, files)) in recycle {
        let result = file_management::recycle_for_state_result(scan_id, files, &resources.files)
            .await
            .and_then(|(message, failed)| {
                if failed == 0 {
                    Ok(message)
                } else {
                    Err(message)
                }
            });
        outcome(core, id, items, "大文件处理", result);
    }
    for (item, name, command) in startup {
        let result = management::set_startup_item(name.clone(), command, false)
            .await
            .map(|_| format!("已关闭 {name} 的开机启动，可在开机管理中恢复。"));
        outcome(core, id, vec![item], "开机优化", result);
    }
    // Leave potentially disruptive network actions until file and startup operations finish.
    for (item, scan_id) in networks {
        let result = network::network_repair(scan_id, None)
            .await
            .and_then(network_outcome);
        outcome(core, id, vec![item], "网络修复", result);
    }
}
fn network_outcome(result: network::RepairResult) -> Result<String, String> {
    let mut message = result.message;
    if result.backup_id.is_some() {
        message.push_str(" 原配置已备份，可在网络修复中还原。");
    }
    if result.restart_required {
        message.push_str(" 请保存工作后重启电脑，重启后再次检查。");
    }
    if let Some(error) = result.verification_error {
        return Err(format!("{message} 复检未完成：{error}"));
    }
    let unresolved = result.scan.as_ref().is_some_and(|scan| {
        !scan.plan.is_empty()
            || matches!(scan.state.as_str(), "repairable" | "attention" | "unknown")
    });
    if let Some(scan) = result.scan {
        message.push_str(&format!(" 复检：{}", scan.summary));
    }
    if result.success && !unresolved {
        Ok(message)
    } else {
        Err(message)
    }
}
#[tauri::command]
pub fn optimization_files(
    report_id: String,
    item_id: String,
    offset: usize,
    state: tauri::State<'_, OptimizationState>,
) -> Result<cleaning::CleaningFiles, String> {
    let (resources, scan_id, groups) = {
        let slot = state.0.session.lock().map_err(|_| "检查状态异常")?;
        let session = slot
            .as_ref()
            .filter(|s| s.report.id == report_id && s.created.elapsed() <= TTL)
            .ok_or("请重新检查")?;
        let Action::Clean { scan_id, groups } =
            session.actions.get(&item_id).ok_or("未找到清理项目")?
        else {
            return Err("此项不是缓存文件".into());
        };
        (session.resources.clone(), scan_id.clone(), groups.clone())
    };
    cleaning::files_for_state(&resources.cleaning, scan_id, groups, offset)
}
#[tauri::command]
pub fn open_health_tool(tool: String) -> Result<(), String> {
    let windows =
        std::path::PathBuf::from(std::env::var_os("SystemRoot").ok_or("无法定位系统目录")?)
            .join("System32");
    let (program, args) = match tool.as_str() {
        "open_devices" => (
            windows.join("mmc.exe"),
            vec![windows.join("devmgmt.msc").to_string_lossy().into()],
        ),
        "open_reliability" => (windows.join("perfmon.exe"), vec!["/rel".into()]),
        _ => return Err("未知系统工具".into()),
    };
    crate::component_launcher::launch(&program, &args)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_recommendations_preserve_network_sync_and_unknown_apps() {
        for app in [
            "Clash.exe",
            "XSUS.exe",
            "OneDrive.exe",
            "ToDesk.exe",
            "security.exe",
            "unknown.exe",
            "notsteam.exe",
        ] {
            assert_eq!(
                startup_suggestion(
                    "Steam",
                    &format!(r#""C:\Apps\{app}" --startup"#),
                    true,
                    true
                )
                .1,
                false
            );
        }
        assert!(startup_suggestion("game", r#""C:\Apps\Steam.exe" -silent"#, true, true).2);
        assert!(!startup_suggestion("game", r#""C:\Apps\Steam.exe" -silent"#, true, false).1);
        assert!(!startup_suggestion("game", r#""C:\Apps\Steam.exe" -silent"#, false, true).1);
    }
    #[test]
    fn plan_requires_confirmation_current_token_and_valid_unique_action_ids() {
        let mut session = Session {
            report: blank_report(),
            created: Instant::now(),
            actions: BTreeMap::new(),
            resources: Arc::new(Resources::default()),
        };
        session.report.phase = "ready".into();
        session.actions.insert(
            "test".into(),
            Action::Startup {
                name: "Test".into(),
                command: "test.exe".into(),
            },
        );
        assert!(approved_actions(&session, &["test".into()], false).is_err());
        assert!(approved_actions(&session, &["unknown".into()], true).is_err());
        assert!(approved_actions(&session, &["test".into(), "test".into()], true).is_err());
        assert!(approved_actions(&session, &["test".into()], true).is_ok());
        session.report.phase = "optimizing".into();
        assert!(approved_actions(&session, &["test".into()], true).is_err());
        session.report.phase = "ready".into();
        session.created = Instant::now() - TTL - Duration::from_secs(1);
        assert!(approved_actions(&session, &["test".into()], true).is_err());
    }
    #[test]
    fn late_or_replaced_scans_cannot_overwrite_new_results() {
        let core = Core::default();
        let report = blank_report();
        let id = report.id.clone();
        *core.session.lock().unwrap() = Some(Session {
            report,
            created: Instant::now(),
            actions: BTreeMap::new(),
            resources: Arc::new(Resources::default()),
        });
        publish(&core, "old", "health", Err("wrong".into()));
        assert_eq!(status(&core, &id, None).unwrap().unwrap().revision, 0);
        publish(&core, &id, "health", Err("denied".into()));
        let report = status(&core, &id, None).unwrap().unwrap();
        assert_eq!(report.sections[2].status, "error");
        assert_eq!(report.sections[0].status, "checking");
        assert!(status(&core, &id, Some(report.revision)).unwrap().is_none());
    }
    #[test]
    fn large_file_suggestions_never_mark_personal_or_application_data_as_junk() {
        let file = file_management::FileRow {
            id: 0,
            path: r"C:\Downloads\setup.exe".into(),
            name: "setup.exe".into(),
            size_bytes: 200 * 1024 * 1024,
            group: None,
            can_recycle: true,
            modified_at: 0,
        };
        assert!(file_suggestion(
            &file,
            Some(std::path::Path::new(r"C:\Downloads")),
            40 * 86400
        )
        .contains("确认不再需要"));
        let personal = file_management::FileRow {
            name: "important.pdf".into(),
            path: r"C:\Downloads\important.pdf".into(),
            ..file.clone()
        };
        assert!(file_suggestion(
            &personal,
            Some(std::path::Path::new(r"C:\Downloads")),
            40 * 86400
        )
        .contains("不代表垃圾"));
    }
    #[test]
    fn cache_files_cannot_also_be_recycled_by_the_same_plan() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("cache.bin");
        std::fs::write(&path, b"cache").unwrap();
        filetime::set_file_mtime(
            &path,
            filetime::FileTime::from_system_time(
                SystemTime::now() - Duration::from_secs(8 * 86400),
            ),
        )
        .unwrap();
        let resources = Arc::new(Resources::default());
        cleaning::fixture_scan(root.path(), &resources.cleaning);
        let mut report = blank_report();
        let mut row = item("file", "cache.bin", "large", "optional");
        row.icon_target = Some(path.to_string_lossy().into());
        enable(&mut row, "移到回收站", false);
        report.sections[1].items.push(row);
        let mut actions = BTreeMap::new();
        actions.insert(
            "file".into(),
            Action::Recycle {
                scan_id: "test".into(),
                file: 0,
            },
        );
        let mut session = Session {
            report,
            created: Instant::now(),
            actions,
            resources,
        };
        deduplicate_cache_files(&mut session);
        assert!(!session.report.sections[1].items[0].selectable);
        assert!(!session.actions.contains_key("file"));
        assert!(path.exists());
    }
    #[test]
    fn network_repair_reports_failed_verification_instead_of_full_success() {
        let result = network::RepairResult {
            success: true,
            message: "commands completed".into(),
            logs: vec![],
            backup_id: Some("backup".into()),
            restart_required: false,
            scan: None,
            verification_error: Some("probe failed".into()),
        };
        let message = network_outcome(result).unwrap_err();
        assert!(message.contains("probe failed"));
        assert!(message.contains("备份"));
        let scan = network::Scan {
            id: "scan".into(),
            checked_at: String::new(),
            state: "repairable".into(),
            summary: "仍有网络问题".into(),
            issues: vec![],
            plan: vec![],
            raw: serde_json::Value::Null,
            probes: vec![],
        };
        let result = network::RepairResult {
            success: true,
            message: "commands completed".into(),
            logs: vec![],
            backup_id: None,
            restart_required: false,
            scan: Some(scan),
            verification_error: None,
        };
        assert!(network_outcome(result)
            .unwrap_err()
            .contains("仍有网络问题"));
    }
    #[test]
    fn approved_plan_cleans_fixture_groups_once_and_records_partial_failure() {
        let root = tempfile::tempdir().unwrap();
        for name in ["chosen.tmp", "chosen.log"] {
            let path = root.path().join(name);
            std::fs::write(&path, b"fixture").unwrap();
            filetime::set_file_mtime(
                &path,
                filetime::FileTime::from_system_time(
                    SystemTime::now() - Duration::from_secs(8 * 86400),
                ),
            )
            .unwrap();
        }
        let recent = root.path().join("personal.txt");
        std::fs::write(&recent, b"keep").unwrap();
        let resources = Arc::new(Resources::default());
        let scan = cleaning::fixture_scan(root.path(), &resources.cleaning);
        let mut actions = BTreeMap::new();
        for (index, group) in scan.groups.iter().enumerate() {
            actions.insert(
                format!("clean-{index}"),
                Action::Clean {
                    scan_id: scan.scan_id.clone(),
                    groups: vec![group.id.clone()],
                },
            );
        }
        actions.insert(
            "stale-file".into(),
            Action::Recycle {
                scan_id: "invalid-scan".into(),
                file: 0,
            },
        );
        let core = Arc::new(Core::default());
        let mut report = blank_report();
        report.phase = "ready".into();
        let id = report.id.clone();
        let selected = actions.keys().cloned().collect::<Vec<_>>();
        *core.session.lock().unwrap() = Some(Session {
            report,
            created: Instant::now(),
            actions,
            resources,
        });
        assert!(start_apply(core.clone(), id.clone(), selected.clone(), false).is_err());
        assert!(root.path().join("chosen.tmp").exists());
        start_apply(core.clone(), id.clone(), selected.clone(), true).unwrap();
        let started = Instant::now();
        loop {
            let report = status(&core, &id, None).unwrap().unwrap();
            if report.phase == "complete" {
                assert_eq!(report.completed_actions, 3);
                assert_eq!(report.outcomes.len(), 2);
                assert_eq!(report.outcomes[0].status, "success");
                assert_eq!(report.outcomes[1].status, "failed");
                break;
            }
            assert!(started.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!root.path().join("chosen.tmp").exists());
        assert!(!root.path().join("chosen.log").exists());
        assert!(recent.exists());
        assert!(start_apply(core, id, selected, true).is_err());
    }
    #[test]
    #[ignore = "read-only unified local disk, cleanup, health, startup and network scan; never optimizes"]
    fn complete_current_machine_check_is_read_only() {
        tauri::async_runtime::block_on(async {
            let resources = Arc::new(Resources::default());
            let core = Arc::new(Core::default());
            let report = blank_report();
            let id = report.id.clone();
            *core.session.lock().unwrap() = Some(Session {
                report,
                created: Instant::now(),
                actions: BTreeMap::new(),
                resources: resources.clone(),
            });
            run_check(core.clone(), id.clone(), resources).await;
            let report = status(&core, &id, None).unwrap().unwrap();
            assert_eq!(report.phase, "ready");
            for section in report.sections {
                println!(
                    "{}：{}；{} 项",
                    section.title,
                    section.summary,
                    section.items.len()
                );
                assert_ne!(section.status, "checking");
            }
            assert!(report.outcomes.is_empty());
        });
    }
}

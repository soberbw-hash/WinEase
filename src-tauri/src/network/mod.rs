//! Functional integration of soberbw-hash/network-first-aid; UI stays native to WinEase.
//! The owner's explicit integration request authorizes reuse; original license is retained.
mod analysis;
pub(crate) mod runtime;
pub mod speed;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
    time::Instant,
};

const COMMON: &str = include_str!("../../resources/network/common.ps1");
const DIAGNOSTICS: &str = include_str!("../../resources/network/diagnostics.ps1");
const CONNECTIVITY: &str = include_str!("../../resources/network/connectivity.ps1");
const REPAIR: &str = include_str!("../../resources/network/repair.ps1");
const RESTORE: &str = include_str!("../../resources/network/restore.ps1");
static LOCK: Mutex<()> = Mutex::new(());
static SCAN: Mutex<Vec<(Instant, Scan)>> = Mutex::new(Vec::new());
pub fn arr<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value[key].as_array().map(Vec::as_slice).unwrap_or(&[])
}
pub fn powershell_path() -> PathBuf {
    PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into()))
        .join(r"System32\WindowsPowerShell\v1.0\powershell.exe")
}
fn root() -> Result<PathBuf, String> {
    let root = crate::local_app_data_dir()
        .ok_or("无法定位网络数据目录")?
        .join("WinToolbox")
        .join("Network");
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    if !crate::cleaning::no_reparse_ancestors(&root) {
        return Err("网络数据目录包含链接".into());
    }
    Ok(root)
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Probe {
    pub id: String,
    pub name: String,
    pub target: String,
    pub ok: bool,
    pub reachable: bool,
    pub portal: bool,
    pub latency_ms: u64,
    pub detail: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub repairable: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub id: String,
    pub title: String,
    pub description: String,
    pub admin: bool,
    pub restart: bool,
    pub disruptive: bool,
    pub scopes: Vec<String>,
    pub limitation: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scan {
    pub id: String,
    pub checked_at: String,
    pub state: String,
    pub summary: String,
    pub issues: Vec<Issue>,
    pub plan: Vec<Tool>,
    pub raw: Value,
    pub probes: Vec<Probe>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub success: bool,
    pub logs: Vec<String>,
    pub error: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairResult {
    pub success: bool,
    pub message: String,
    pub logs: Vec<String>,
    pub backup_id: Option<String>,
    pub restart_required: bool,
    pub scan: Option<Scan>,
    pub verification_error: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stored {
    schema: u8,
    id: String,
    created_at: String,
    actions: Vec<String>,
    user_sid: String,
    raw: Value,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub id: String,
    pub created_at: String,
    pub title: String,
    pub can_restore: bool,
    pub limitation: String,
}
pub fn tools() -> Vec<Tool> {
    [
        (
            "flush-dns",
            "刷新 DNS 缓存",
            "清除解析缓存，保留 DNS 服务器。",
            false,
            false,
            false,
            "",
            "缓存无法还原，不更改网络配置。",
        ),
        (
            "disable-dead-proxy",
            "关闭失效代理",
            "仅在全部本机代理端口无法连接且没有 PAC/组织策略时关闭。",
            false,
            false,
            false,
            "proxy-enable",
            "可还原代理启用状态。",
        ),
        (
            "remove-orphan-tun-routes",
            "清理残留隧道路由",
            "仅清理确认断开的虚拟隧道默认与分流路由。",
            true,
            false,
            false,
            "routes",
            "可按网卡 GUID 还原删除前的路由。",
        ),
        (
            "renew-dhcp",
            "重新获取 IP",
            "只更新 DHCP 物理网卡，保留静态 IP；不会先释放全机地址。",
            true,
            false,
            true,
            "",
            "DHCP 租约无法回滚，网络可能短暂中断。",
        ),
        (
            "dns-auto",
            "恢复自动 DNS",
            "将已连接的 DHCP 物理网卡 DNS 恢复自动获取。",
            true,
            false,
            false,
            "dns",
            "可按网卡 GUID 还原 IPv4/IPv6 的自动或手动 DNS。",
        ),
        (
            "reset-winhttp-proxy",
            "清除后台代理",
            "将 WinHTTP 静态代理改为直连；不会解决所有客户端代理问题。",
            true,
            false,
            false,
            "winhttp",
            "仅支持静态 WinHTTP 配置的还原；高级自动代理不修改。",
        ),
        (
            "sync-winhttp-proxy",
            "同步后台代理",
            "将当前用户的手动 HTTP 代理应用到 WinHTTP。",
            true,
            false,
            false,
            "winhttp",
            "不支持 PAC/SOCKS 导入；只影响使用 WinHTTP 的程序。",
        ),
        (
            "normalize-proxy-bypass",
            "局域网直连",
            "保留代理例外，补充本机与私有网段。",
            false,
            false,
            false,
            "proxy-bypass",
            "可还原原有代理例外。",
        ),
        (
            "restart-active-adapters",
            "重启网卡",
            "重启已连接的物理网卡，网络会暂时中断。",
            true,
            false,
            true,
            "",
            "网卡启停不是配置回滚，使用远程桌面时不可执行。",
        ),
        (
            "reset-winsock",
            "重置 Winsock",
            "重建套接字目录，第三方网络扩展可能需要重装。",
            true,
            true,
            true,
            "",
            "无法通过快照还原套接字目录。",
        ),
        (
            "reset-tcpip",
            "重置 TCP/IP",
            "重置 IPv4/IPv6，可能移除静态 IP 和自定义接口配置。",
            true,
            true,
            true,
            "",
            "配置快照不能完整还原网络栈，单位或静态 IP 网络不可执行。",
        ),
        (
            "reset-firewall",
            "重置防火墙",
            "备份策略后清除自定义规则并恢复 Windows 默认规则。",
            true,
            false,
            true,
            "firewall",
            "可导入原有防火墙策略；受组织策略控制时不可执行。",
        ),
        (
            "reset-hosts",
            "重置 Hosts",
            "备份后移除所有自定义 Hosts 记录。",
            true,
            false,
            false,
            "hosts",
            "可还原原文件；会影响本地开发和自定义域名。",
        ),
        (
            "full-network-reset",
            "重装网络组件",
            "最后手段：移除并重新安装网卡组件，需要重启。",
            true,
            true,
            true,
            "",
            "无法完整回滚；VPN、虚拟网卡可能需要重新安装。",
        ),
    ]
    .into_iter()
    .map(
        |(id, title, description, admin, restart, disruptive, scope, limitation)| Tool {
            id: id.into(),
            title: title.into(),
            description: description.into(),
            admin,
            restart,
            disruptive,
            scopes: if scope.is_empty() {
                vec![]
            } else {
                vec![scope.into()]
            },
            limitation: limitation.into(),
        },
    )
    .collect()
}
fn tool(id: &str) -> Result<Tool, String> {
    tools()
        .into_iter()
        .find(|t| t.id == id)
        .ok_or_else(|| "修复动作不在白名单内".into())
}
fn capture(root: &Path) -> Result<Value, String> {
    let output = runtime::run(root, &format!("{COMMON}\n{DIAGNOSTICS}"), 65)?;
    let raw: Value = serde_json::from_str(&output).map_err(|e| format!("网络配置读取失败：{e}"))?;
    let values = arr(&raw, "proxyValues");
    if values.len() != 5
        || values
            .iter()
            .any(|v| !v.is_object() || v["name"].as_str().is_none())
    {
        return Err("代理备份数据结构异常，已停止检查与修复".into());
    }
    Ok(raw)
}
fn scan(root: &Path) -> Result<Scan, String> {
    let raw = capture(root)?;
    let endpoints = if raw["proxy"]["enabled"] == true {
        analysis::endpoints(raw["proxy"]["server"].as_str().unwrap_or("")).unwrap_or_default()
    } else {
        vec![]
    };
    let http = endpoints
        .iter()
        .find(|e| e.scheme == "https")
        .or_else(|| endpoints.iter().find(|e| e.scheme == "http"))
        .map(|e| {
            format!(
                "http://{}:{}",
                if e.host == "::1" { "[::1]" } else { &e.host },
                e.port
            )
        })
        .unwrap_or_default();
    let script = CONNECTIVITY
        .replace(
            "__ENDPOINTS_JSON__",
            &runtime::quote(&serde_json::to_string(&endpoints).map_err(|e| e.to_string())?),
        )
        .replace("__HTTP_PROXY__", &runtime::quote(&http));
    let output = runtime::run(root, &format!("{COMMON}\n{script}"), 60)?;
    let probes = serde_json::from_str(&output).map_err(|e| format!("连通性检测结果无效：{e}"))?;
    Ok(analysis::analyze(raw, probes, &tools()))
}
fn remember(scan: &Scan) {
    if let Ok(mut cache) = SCAN.lock() {
        cache.retain(|(time, item)| time.elapsed().as_secs() <= 600 && item.id != scan.id);
        if cache.len() >= 8 {
            cache.remove(0);
        }
        cache.push((Instant::now(), scan.clone()));
    }
}
fn snapshot_path(root: &Path, id: &str) -> Result<PathBuf, String> {
    if uuid::Uuid::parse_str(id).is_err() {
        return Err("备份编号无效".into());
    }
    let path = root.join("backups").join(id);
    let existing = path
        .ancestors()
        .find(|p| p.exists())
        .ok_or("备份目录无效")?;
    if !crate::cleaning::no_reparse_ancestors(existing) {
        return Err("备份路径包含链接".into());
    }
    Ok(path)
}
fn save(root: &Path, raw: Value, actions: Vec<String>) -> Result<Stored, String> {
    if !arr(&raw, "collectionErrors").is_empty() {
        return Err("网络配置未完整读取，已停止写入和修复".into());
    }
    let id = uuid::Uuid::new_v4().to_string();
    let path = snapshot_path(root, &id)?;
    fs::create_dir_all(path.parent().ok_or("备份路径异常")?).map_err(|e| e.to_string())?;
    fs::create_dir(&path).map_err(|e| e.to_string())?;
    if !crate::cleaning::no_reparse_ancestors(&path) {
        return Err("备份目录包含链接".into());
    }
    let hosts =
        PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into()))
            .join(r"System32\drivers\etc\hosts");
    if !crate::cleaning::no_reparse_ancestors(&hosts) {
        return Err("Hosts 路径包含链接".into());
    }
    fs::copy(hosts, path.join("hosts")).map_err(|e| format!("Hosts 备份失败，未执行修复：{e}"))?;
    let stored = Stored {
        schema: 2,
        id: id.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
        actions,
        user_sid: raw["userSid"].as_str().ok_or("用户信息未读取")?.into(),
        raw,
    };
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path.join("snapshot.json"))
        .map_err(|e| e.to_string())?;
    file.write_all(&serde_json::to_vec(&stored).map_err(|e| e.to_string())?)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(stored)
}
fn read(root: &Path, id: &str) -> Result<Stored, String> {
    let path = snapshot_path(root, id)?.join("snapshot.json");
    if !crate::cleaning::no_reparse_ancestors(&path) {
        return Err("备份文件包含链接".into());
    }
    if fs::metadata(&path).map_err(|e| e.to_string())?.len() > 2_000_000 {
        return Err("备份内容过大".into());
    }
    let stored: Stored = serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if stored.schema != 2 || stored.id != id {
        return Err("备份格式不支持".into());
    }
    for action in &stored.actions {
        if action != "manual" {
            tool(action)?;
        }
    }
    Ok(stored)
}
fn scopes(stored: &Stored) -> Vec<String> {
    if stored.actions == ["manual"] {
        return vec![
            "proxy".into(),
            "dns".into(),
            "hosts".into(),
            "winhttp".into(),
        ];
    }
    let mut values = Vec::new();
    for id in &stored.actions {
        if let Ok(tool) = tool(id) {
            for s in tool.scopes {
                if !values.contains(&s) {
                    values.push(s);
                }
            }
        }
    }
    values
}
fn audit(root: &Path, operation: &str, result: &RepairResult) {
    if let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join("audit.jsonl"))
    {
        let _ = writeln!(
            file,
            "{}",
            json!({"at":chrono::Utc::now().to_rfc3339(),"operation":operation,"success":result.success,"message":result.message,"backupId":result.backup_id,"restartRequired":result.restart_required,"logs":result.logs})
        );
    }
}
fn execute_body(
    root: &Path,
    operation: &str,
    action: &str,
    stored: &Stored,
) -> Result<Outcome, String> {
    let path = snapshot_path(root, &stored.id)?;
    for name in ["snapshot.json", "hosts", "firewall.wfw"] {
        let file = path.join(name);
        if file.exists() && !crate::cleaning::no_reparse_ancestors(&file) {
            return Err("备份文件包含链接，已停止操作".into());
        }
    }
    let hosts =
        PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into()))
            .join(r"System32\drivers\etc\hosts");
    if !crate::cleaning::no_reparse_ancestors(&hosts) {
        return Err("系统 Hosts 路径包含链接".into());
    }
    if operation != "restore" {
        tool(action)?;
    }
    let sid = stored.user_sid.clone();
    let script = if operation == "restore" {
        RESTORE
    } else {
        REPAIR
    };
    let prelude=format!("$snapshotPath='{}'\n$snapshotDirectory='{}'\n$actionId='{}'\n$expectedSid='{}'\n$scopes='{}' | ConvertFrom-Json\n$logs=[Collections.Generic.List[string]]::new()\nfunction Add-Log([string]$text){{$logs.Add($text)}}\n",runtime::quote(&path.join("snapshot.json").to_string_lossy()),runtime::quote(&path.to_string_lossy()),runtime::quote(action),runtime::quote(&sid),runtime::quote(&serde_json::to_string(&scopes(stored)).map_err(|e|e.to_string())?));
    let body=format!("{COMMON}\n{prelude}\ntry {{\nif([Security.Principal.WindowsIdentity]::GetCurrent().User.Value -ne $expectedSid){{throw '请使用当前 Windows 账户授权；不能通过其他管理员账户修改当前用户配置'}}\n$live=(& {{\n{DIAGNOSTICS}\n}})|ConvertFrom-Json\n{script}\n@{{success=$true;logs=@($logs);error=$null}}|ConvertTo-Json -Depth 6 -Compress\n}}catch{{@{{success=$false;logs=@($logs);error=$_.Exception.Message}}|ConvertTo-Json -Depth 6 -Compress}}");
    let output = runtime::run(root, &body, 180)?;
    serde_json::from_str(&output).map_err(|e| format!("修复结果无效：{e}"))
}
fn unfinished(root: &Path) -> Result<(), String> {
    if runtime::pending(root) {
        Err("上一次管理员任务尚未返回结果。请等待完成后再操作。".into())
    } else {
        Ok(())
    }
}
#[tauri::command]
pub async fn network_tools() -> Vec<Tool> {
    tools()
}
#[tauri::command]
pub async fn network_scan() -> Result<Scan, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let _guard = LOCK.try_lock().map_err(|_| "网络操作正在进行，请稍候")?;
        let root = root()?;
        let scan = scan(&root)?;
        remember(&scan);
        Ok(scan)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn network_backups() -> Result<Vec<Backup>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let root = root()?;
        let mut results = Vec::new();
        if let Ok(entries) = fs::read_dir(root.join("backups")) {
            for entry in entries.flatten() {
                let id = entry.file_name().to_string_lossy().to_string();
                if let Ok(stored) = read(&root, &id) {
                    let expected_scopes = scopes(&stored);
                    let files_available = (!expected_scopes.iter().any(|s| s == "firewall")
                        || entry.path().join("firewall.wfw").is_file())
                        && (!expected_scopes.iter().any(|s| s == "hosts")
                            || entry.path().join("hosts").is_file());
                    let can_restore = !expected_scopes.is_empty() && files_available;
                    let descriptions: Vec<_> = stored
                        .actions
                        .iter()
                        .map(|id| {
                            if id == "manual" {
                                "手动备份".into()
                            } else {
                                tool(id).map(|t| t.title).unwrap_or_default()
                            }
                        })
                        .collect();
                    let limitations: Vec<_> = stored
                        .actions
                        .iter()
                        .filter_map(|id| tool(id).ok().map(|t| t.limitation))
                        .collect();
                    results.push(Backup {
                        id,
                        created_at: stored.created_at,
                        title: descriptions.join("、"),
                        can_restore,
                        limitation: if stored.actions == ["manual"] {
                            "还原用户代理、DNS、Hosts 与静态 WinHTTP；不含网卡组件和网络栈。".into()
                        } else {
                            limitations.join(" ")
                        },
                    });
                }
            }
        }
        results.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(results)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn network_backup() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let _guard = LOCK.try_lock().map_err(|_| "网络操作正在进行")?;
        let root = root()?;
        unfinished(&root)?;
        let stored = save(&root, capture(&root)?, vec!["manual".into()])?;
        Ok(stored.id)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn network_repair(
    scan_id: String,
    action_id: Option<String>,
) -> Result<RepairResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = LOCK
            .try_lock()
            .map_err(|_| "网络操作正在进行，请勿重复修复")?;
        let root = root()?;
        unfinished(&root)?;
        let approved = {
            let cache = SCAN.lock().map_err(|_| "检查状态异常")?;
            let (_, scan) = cache
                .iter()
                .find(|(time, item)| time.elapsed().as_secs() <= 600 && item.id == scan_id)
                .ok_or("检查结果已过期，请重新检查网络")?;
            scan.clone()
        };
        let current = scan(&root)?;
        let automatic = action_id.is_none();
        let actions = if let Some(id) = action_id {
            vec![tool(&id)?]
        } else {
            current
                .plan
                .iter()
                .filter(|t| approved.plan.iter().any(|a| a.id == t.id))
                .cloned()
                .collect::<Vec<_>>()
        };
        if actions.is_empty() {
            remember(&current);
            return Ok(RepairResult {
                success: true,
                message: "重新核实后没有需要自动修复的配置。".into(),
                logs: vec![],
                backup_id: None,
                restart_required: false,
                scan: Some(current),
                verification_error: None,
            });
        }
        if !arr(&current.raw, "collectionErrors").is_empty() {
            return Err("配置读取不完整，请重新检查后再修复".into());
        }
        let mut backup_raw = current.raw.clone();
        backup_raw["automaticRepair"] = json!(automatic);
        let stored = save(
            &root,
            backup_raw,
            actions.iter().map(|t| t.id.clone()).collect(),
        )?;
        let mut result = RepairResult {
            success: true,
            message: String::new(),
            logs: vec![],
            backup_id: Some(stored.id.clone()),
            restart_required: false,
            scan: None,
            verification_error: None,
        };
        for action in actions {
            let outcome = if action.admin {
                runtime::elevated(&root, "repair", &action.id, &stored.user_sid, &stored.id)
            } else {
                execute_body(&root, "repair", &action.id, &stored)
            };
            match outcome {
                Ok(outcome) => {
                    result.logs.extend(outcome.logs);
                    if !outcome.success {
                        result.success = false;
                        result.message = outcome.error.unwrap_or("修复未完成".into());
                        break;
                    }
                    result.restart_required |= action.restart;
                }
                Err(error) => {
                    result.success = false;
                    result.message = error;
                    break;
                }
            }
        }
        if result.message.is_empty() {
            result.message = if result.restart_required {
                "操作完成，请重启电脑后重新检查网络。"
            } else {
                "操作完成，已自动重新检查网络。"
            }
            .into();
        }
        if !runtime::pending(&root) {
            match scan(&root) {
                Ok(scan) => {
                    remember(&scan);
                    result.scan = Some(scan);
                }
                Err(error) => {
                    result.verification_error = Some(error);
                    result.message.push_str(" 复检未完成，请稍后重新检查。");
                }
            }
        }
        audit(&root, "repair", &result);
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn network_restore(backup_id: String) -> Result<RepairResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = LOCK.try_lock().map_err(|_| "网络操作正在进行")?;
        let root = root()?;
        unfinished(&root)?;
        let stored = read(&root, &backup_id)?;
        if scopes(&stored).is_empty() {
            return Err("此操作不修改可恢复的配置，不能通过快照回滚".into());
        }
        let current = capture(&root)?;
        if current["userSid"] != stored.user_sid {
            return Err("此备份不属于当前账户".into());
        }
        let rollback = save(&root, current, vec!["manual".into()])?;
        let outcome = runtime::elevated(&root, "restore", "restore", &stored.user_sid, &stored.id);
        let mut result = match outcome {
            Ok(o) => RepairResult {
                success: o.success,
                message: o
                    .error
                    .unwrap_or("已还原本次修复涉及的配置，并自动复检。".into()),
                logs: o.logs,
                backup_id: Some(rollback.id),
                restart_required: false,
                scan: None,
                verification_error: None,
            },
            Err(e) => RepairResult {
                success: false,
                message: e,
                logs: vec![],
                backup_id: Some(rollback.id),
                restart_required: false,
                scan: None,
                verification_error: None,
            },
        };
        if !runtime::pending(&root) {
            match scan(&root) {
                Ok(scan) => {
                    remember(&scan);
                    result.scan = Some(scan);
                }
                Err(e) => {
                    result.verification_error = Some(e);
                }
            }
        }
        audit(&root, "restore", &result);
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn network_export_report(scan_id: String) -> Result<String, String> {
    let cache = SCAN.lock().map_err(|_| "检测状态异常")?;
    let (_, scan) = cache
        .iter()
        .find(|(time, s)| s.id == scan_id && time.elapsed().as_secs() <= 600)
        .ok_or("请先检查网络")?;
    let directory = root()?.join("reports");
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let path = directory.join(format!(
        "network-{}.json",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    ));
    // Do not export PAC URLs, proxy strings, account identifiers, IPs, Hosts content or process paths.
    let data = json!({"product":"WinEase","checkedAt":scan.checked_at,"state":scan.state,"summary":scan.summary,"issues":scan.issues,"tests":scan.probes.iter().map(|p|json!({"name":p.name,"ok":p.ok,"reachable":p.reachable,"portal":p.portal,"latencyMs":p.latency_ms})).collect::<Vec<_>>()});
    fs::write(
        &path,
        serde_json::to_vec_pretty(&data).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into())
}
pub fn helper_entry() -> bool {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) != Some("--winease-network-helper") {
        return false;
    }
    if args.len() != 8 {
        return true;
    }
    let operation = &args[2];
    let action = &args[3];
    let sid = &args[4];
    let root = PathBuf::from(&args[5]);
    let job = &args[6];
    let id = &args[7];
    if !["repair", "restore"].contains(&operation.as_str())
        || uuid::Uuid::parse_str(job).is_err()
        || !sid.starts_with("S-1-5-")
        || !root.ends_with(r"WinToolbox\Network")
        || !root.is_absolute()
        || !crate::cleaning::no_reparse_ancestors(&root)
    {
        return true;
    }
    let directory = root.join("admin-jobs").join(job);
    if !directory.join("pending").is_file() || !crate::cleaning::no_reparse_ancestors(&directory) {
        return true;
    }
    let result = (|| {
        let stored = read(&root, id)?;
        if stored.user_sid != *sid {
            return Err("任务账户不一致".into());
        }
        if operation == "repair" && !stored.actions.iter().any(|a| a == action) {
            return Err("修复不属于该备份任务".into());
        }
        execute_body(&root, operation, action, &stored)
    })();
    let outcome = result.unwrap_or_else(|error| Outcome {
        success: false,
        logs: vec![],
        error: Some(error),
    });
    if let Ok(data) = serde_json::to_vec(&outcome) {
        let temp = directory.join("result.tmp");
        if fs::write(&temp, data).is_ok() {
            let _ = fs::rename(temp, directory.join("result.json"));
        }
    }
    true
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshots_cannot_escape_root() {
        let directory = tempfile::tempdir().unwrap();
        for id in ["../Windows", "bad", "..\\audit.jsonl"] {
            assert!(snapshot_path(directory.path(), id).is_err());
        }
    }
    #[test]
    fn unknown_actions_and_nonrestorable_operations_are_explicit() {
        assert!(tool("powershell-anything").is_err());
        for id in [
            "reset-winsock",
            "reset-tcpip",
            "full-network-reset",
            "renew-dhcp",
        ] {
            assert!(tool(id).unwrap().scopes.is_empty());
        }
    }
    #[test]
    fn backup_roundtrip_preserves_dns_mode_and_restores_only_changed_scope() {
        let directory = tempfile::tempdir().unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let path = snapshot_path(directory.path(), &id).unwrap();
        fs::create_dir_all(&path).unwrap();
        let fixture = Stored {
            schema: 2,
            id: id.clone(),
            created_at: "now".into(),
            actions: vec!["disable-dead-proxy".into()],
            user_sid: "S-1-5-21-test".into(),
            raw: json!({"dns":[{"automatic":true,"guid":"original-guid"}],"proxyValues":[{"name":"ProxyEnable","exists":false}]}),
        };
        fs::write(
            path.join("snapshot.json"),
            serde_json::to_vec(&fixture).unwrap(),
        )
        .unwrap();
        let stored = read(directory.path(), &id).unwrap();
        assert_eq!(stored.raw["dns"][0]["automatic"], true);
        assert_eq!(scopes(&stored), vec!["proxy-enable"]);
    }
    #[test]
    fn powershell_literals_escape_quotes_without_interpolation() {
        assert_eq!(runtime::quote("a'b$();c"), "a''b$();c");
    }
    #[test]
    fn runtime_supports_unicode_and_rejects_script_failure() {
        let directory = tempfile::tempdir().unwrap();
        assert_eq!(
            runtime::run(directory.path(), "'中文网络任务'", 5).unwrap(),
            "中文网络任务"
        );
        assert!(runtime::run(directory.path(), "throw 'isolated failure'", 5).is_err());
    }
    #[test]
    fn different_admin_account_is_rejected_before_network_access() {
        let directory = tempfile::tempdir().unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let path = snapshot_path(directory.path(), &id).unwrap();
        fs::create_dir_all(&path).unwrap();
        let stored = Stored {
            schema: 2,
            id,
            created_at: "test".into(),
            actions: vec!["flush-dns".into()],
            user_sid: "S-1-5-21-0-0-0-0000".into(),
            raw: json!({}),
        };
        let outcome = execute_body(directory.path(), "repair", "flush-dns", &stored).unwrap();
        assert!(!outcome.success);
        assert!(outcome.error.unwrap().contains("当前 Windows 账户"));
        assert!(outcome.logs.is_empty());
    }
    #[test]
    #[ignore = "read-only integration test: probes current network; never repairs"]
    fn reads_current_network_without_mutation() {
        let directory = tempfile::tempdir().unwrap();
        let before = capture(directory.path()).unwrap();
        let current = scan(directory.path()).unwrap();
        let after = capture(directory.path()).unwrap();
        for key in ["proxyValues", "dns", "routes"] {
            assert_eq!(
                before[key], after[key],
                "read-only diagnostics changed {key}"
            );
        }
        let output = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("output/network-live-scan.json");
        fs::write(output, serde_json::to_vec_pretty(&current).unwrap()).unwrap();
        println!(
            "{}; {} probes; {} automatic repair steps",
            current.summary,
            current.probes.len(),
            current.plan.len()
        );
        for probe in current.probes {
            println!(
                "{}: ok={} reachable={}",
                probe.name, probe.ok, probe.reachable
            );
        }
    }
}

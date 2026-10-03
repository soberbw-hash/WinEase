//! Read-only, evidence-based health checks. Unknown data is never reported as healthy.
use serde::Serialize;
use serde_json::Value;
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheck {
    pub id: String,
    pub title: String,
    pub status: String,
    pub detail: String,
    pub target: Option<String>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthReport {
    pub checked_at: String,
    pub checks: Vec<HealthCheck>,
}
pub async fn check() -> Result<HealthReport, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let directory = crate::local_app_data_dir()
            .ok_or("无法读取用户目录")?
            .join("WinEase/Health");
        let output = crate::network::runtime::run(
            &directory,
            include_str!("../resources/health-check.ps1"),
            60,
        )?;
        let raw: Value =
            serde_json::from_str(&output).map_err(|error| format!("体检数据无效：{error}"))?;
        Ok(analyze(&raw))
    })
    .await
    .map_err(|error| error.to_string())?
}
fn row(id: &str, title: &str, status: &str, detail: String, target: Option<&str>) -> HealthCheck {
    HealthCheck {
        id: id.into(),
        title: title.into(),
        status: status.into(),
        detail,
        target: target.map(Into::into),
    }
}
fn missing(raw: &Value, id: &str) -> Option<String> {
    crate::network::arr(raw, "errors")
        .iter()
        .find(|error| error["name"] == id)
        .map(|error| error["detail"].as_str().unwrap_or("未能读取").into())
        .or_else(|| {
            if ["drives", "providers", "firewall", "devices", "events"].contains(&id)
                && !raw[id].is_array()
            {
                Some("未能完整读取检查数据".into())
            } else {
                None
            }
        })
}
pub(crate) fn analyze(raw: &Value) -> HealthReport {
    let mut checks = Vec::new();
    if let Some(percent) = raw["memoryPercent"].as_f64() {
        checks.push(row("memory", "内存压力", if percent >= 85.0 {"attention"} else {"healthy"},
            if percent >= 85.0 {format!("当前占用 {percent:.0}%；建议在进程管理中关闭不用的应用，清理缓存不会增加物理内存。")}
            else {format!("当前占用 {percent:.0}%")}, Some("open_processes")));
    } else {
        checks.push(row(
            "memory",
            "内存压力",
            "unknown",
            missing(raw, "memory").unwrap_or("未能读取内存状态".into()),
            None,
        ));
    }
    for drive in crate::network::arr(raw, "drives") {
        let total = drive["totalBytes"].as_u64().unwrap_or(0);
        let free = drive["freeBytes"].as_u64().unwrap_or(0);
        if total == 0 {
            continue;
        }
        let name = drive["name"].as_str().unwrap_or("磁盘");
        let low = free < 5 * 1024u64.pow(3)
            || (free < 20 * 1024u64.pow(3) && free as f64 / (total as f64) < 0.1);
        checks.push(row(
            &format!("disk-{name}"),
            &format!("{name} 可用空间"),
            if low { "attention" } else { "healthy" },
            format!(
                "可用 {} / {}{}",
                crate::format_bytes(free),
                crate::format_bytes(total),
                if low {
                    "；建议清理缓存并检查大文件。"
                } else {
                    ""
                }
            ),
            Some("open_storage"),
        ));
    }
    if let Some(error) = missing(raw, "drives") {
        checks.push(row(
            "drives",
            "磁盘空间",
            "unknown",
            error,
            Some("open_storage"),
        ));
    }
    let defender = &raw["defender"];
    let third_party = crate::network::arr(raw, "providers")
        .iter()
        .filter_map(|provider| provider["name"].as_str())
        .filter(|name| !name.to_ascii_lowercase().contains("defender"))
        .collect::<Vec<_>>();
    if defender["enabled"] == true && defender["realtime"] == true {
        checks.push(row(
            "security",
            "病毒防护",
            "healthy",
            "Microsoft Defender 实时防护已开启".into(),
            Some("windowsdefender:"),
        ));
        if let Some(age) = defender["signatureAge"].as_u64() {
            checks.push(row(
                "signatures",
                "病毒库更新",
                if age > 7 { "attention" } else { "healthy" },
                format!(
                    "病毒库距今 {age} 天{}",
                    if age > 7 {
                        "；建议更新病毒库。"
                    } else {
                        ""
                    }
                ),
                Some("windowsdefender://threat/"),
            ));
        } else {
            checks.push(row(
                "signatures",
                "病毒库更新",
                "unknown",
                "未能读取病毒库日期".into(),
                Some("windowsdefender:"),
            ));
        }
    } else if !third_party.is_empty() {
        checks.push(row(
            "security",
            "病毒防护",
            "info",
            format!(
                "已注册 {}；请在安全中心核实是否正常防护。不会重复启用 Defender。",
                third_party.join("、")
            ),
            Some("windowsdefender:"),
        ));
    } else {
        let unknown = defender["enabled"].as_bool().is_none()
            || defender["realtime"].as_bool().is_none()
            || missing(raw, "providers").is_some();
        checks.push(row(
            "security",
            "病毒防护",
            if unknown { "unknown" } else { "attention" },
            if unknown {
                "未能完整读取防护提供程序，请在安全中心确认。".into()
            } else {
                "Defender 实时防护未开启，未检测到其他已注册防护；请在安全中心检查。".into()
            },
            Some("windowsdefender:"),
        ));
    }
    let disabled = crate::network::arr(raw, "firewall")
        .iter()
        .filter(|profile| profile["enabled"] == false)
        .filter_map(|profile| profile["name"].as_str())
        .collect::<Vec<_>>();
    checks.push(if let Some(error) = missing(raw, "firewall") {
        row(
            "firewall",
            "当前网络防火墙",
            "unknown",
            error,
            Some("windowsdefender://network/"),
        )
    } else if crate::network::arr(raw, "firewall")
        .iter()
        .any(|profile| profile["enabled"].as_bool().is_none())
    {
        row(
            "firewall",
            "当前网络防火墙",
            "unknown",
            "未能读取活动配置的防火墙状态".into(),
            Some("windowsdefender://network/"),
        )
    } else if !disabled.is_empty() {
        row(
            "firewall",
            "当前网络防火墙",
            "attention",
            format!(
                "{} 防火墙已关闭；请确认是否由其他安全软件接管。",
                disabled.join("、")
            ),
            Some("windowsdefender://network/"),
        )
    } else if crate::network::arr(raw, "firewall").is_empty() {
        row(
            "firewall",
            "当前网络防火墙",
            "info",
            "没有当前连接的网络配置，暂无法核实活动防火墙。".into(),
            Some("windowsdefender://network/"),
        )
    } else {
        row(
            "firewall",
            "当前网络防火墙",
            "healthy",
            "活动网络配置的防火墙已开启".into(),
            None,
        )
    });
    for (id, title, field, warning, target) in [
        (
            "reboot",
            "待重启状态",
            "pendingReboot",
            "系统更新等待重启，请保存工作后重启电脑。",
            "ms-settings:windowsupdate",
        ),
        (
            "updates",
            "Windows 更新",
            "updatesPaused",
            "更新当前已暂停；建议在设置中核实是否需要恢复。",
            "ms-settings:windowsupdate",
        ),
    ] {
        let value = raw[field].as_bool();
        checks.push(row(
            id,
            title,
            if value.is_none() {
                "unknown"
            } else if value == Some(true) {
                "attention"
            } else {
                "healthy"
            },
            missing(raw, id).unwrap_or_else(|| {
                if value.is_none() {
                    "未能读取配置".into()
                } else if value == Some(true) {
                    warning.into()
                } else {
                    "未发现待处理项".into()
                }
            }),
            Some(target),
        ));
    }
    let devices = crate::network::arr(raw, "devices");
    checks.push(row(
        "devices",
        "设备与驱动状态",
        if missing(raw, "devices").is_some() {
            "unknown"
        } else if devices.is_empty() {
            "healthy"
        } else {
            "attention"
        },
        missing(raw, "devices").unwrap_or_else(|| {
            if devices.is_empty() {
                "未发现异常设备；已排除用户主动禁用的设备。".into()
            } else {
                devices
                    .iter()
                    .map(|d| {
                        format!(
                            "{}（错误 {}）",
                            d["name"].as_str().unwrap_or("未知设备"),
                            d["code"]
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("；")
            }
        }),
        Some("open_devices"),
    ));
    let events = crate::network::arr(raw, "events");
    if let Some(error) = missing(raw, "events") {
        checks.push(row(
            "events",
            "近期系统可靠性",
            "unknown",
            error,
            Some("open_reliability"),
        ));
    } else {
        let crashes = events
            .iter()
            .filter(|e| e["id"] == 41 || e["id"] == 1001)
            .count();
        let disks = events.len() - crashes;
        checks.push(row("events", "近期系统可靠性", if crashes + disks == 0 {"healthy"}else{"attention"},
            if crashes + disks == 0 {"最近 7 天未记录所检查的异常关机、蓝屏或磁盘错误。".into()}
            else {format!("最近 7 天记录异常关机/蓝屏 {crashes} 条、磁盘/文件系统错误 {disks} 条；共读取最多 100 条相关事件。建议查看可靠性记录；磁盘错误应优先备份数据。记录不能直接说明故障原因。")}, Some("open_reliability")));
    }
    HealthReport {
        checked_at: chrono::Utc::now().to_rfc3339(),
        checks,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn failed_probes_are_unknown_and_third_party_protection_is_not_disabled() {
        let result = analyze(
            &json!({"defender":{"enabled":false,"realtime":false},"providers":[{"name":"Other AV"}],"errors":[{"name":"devices","detail":"denied"},{"name":"events","detail":"denied"}]}),
        );
        assert_eq!(
            result
                .checks
                .iter()
                .find(|r| r.id == "security")
                .unwrap()
                .status,
            "info"
        );
        for id in ["devices", "events", "memory", "reboot"] {
            assert_eq!(
                result.checks.iter().find(|r| r.id == id).unwrap().status,
                "unknown"
            );
        }
    }
    #[test]
    fn absent_probe_data_is_never_healthy() {
        let result = analyze(&json!({"defender":{}, "firewall":[{"name":"Public"}]}));
        for id in ["drives", "security", "firewall", "devices", "events"] {
            assert_eq!(
                result.checks.iter().find(|r| r.id == id).unwrap().status,
                "unknown"
            );
        }
    }
    #[test]
    fn genuine_pressure_and_security_findings_are_actionable() {
        let result = analyze(
            &json!({"memoryPercent":92,"drives":[{"name":"C:","totalBytes":100_000_000_000u64,"freeBytes":2_000_000_000u64}],"defender":{"enabled":true,"realtime":true,"signatureAge":20},"firewall":[{"name":"Public","enabled":false}],"devices":[{"name":"Example device","code":28}],"events":[{"id":41}],"pendingReboot":true,"updatesPaused":true}),
        );
        for id in [
            "memory",
            "disk-C:",
            "signatures",
            "firewall",
            "devices",
            "events",
            "reboot",
            "updates",
        ] {
            let check = result.checks.iter().find(|r| r.id == id).unwrap();
            assert_eq!(check.status, "attention");
            assert!(check.target.is_some());
        }
    }
    #[test]
    #[ignore = "read-only local Windows health probes; never repairs or enables protection"]
    fn current_windows_health_is_readable() {
        let result = tauri::async_runtime::block_on(check()).unwrap();
        assert!(result.checks.len() >= 8);
        for item in result.checks {
            println!("{}: {} - {}", item.title, item.status, item.detail);
        }
    }
}

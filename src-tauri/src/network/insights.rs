//! Explicit, bounded network observations. Never changes routes, proxy or DNS.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{net::IpAddr, sync::Mutex};
const ENGINE: &str = include_str!("../../resources/network/insights.cs");
static SITES: Mutex<()> = Mutex::new(());
static IP: Mutex<()> = Mutex::new(());
const GEO: &str =
    "https://ipwho.is/?fields=ip,success,message,type,country,country_code,region,city,connection&lang=zh-CN";
const TRACE: &str = "https://www.cloudflare.com/cdn-cgi/trace";
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Site {
    id: String,
    name: String,
    url: String,
    area: String,
    reachable: bool,
    ok: bool,
    status_code: Option<u16>,
    latency_ms: Option<u64>,
    error: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IpInfo {
    ip: String,
    version: String,
    country: Option<String>,
    region: Option<String>,
    city: Option<String>,
    isp: Option<String>,
    organization: Option<String>,
    asn: Option<u32>,
    domain: Option<String>,
    source: &'static str,
    checked_at: String,
    warning: Option<String>,
}
fn text(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.chars().take(256).collect())
}
fn address(value: &str) -> Result<(String, String), String> {
    let ip: IpAddr = value.parse().map_err(|_| "IP 服务返回无效地址")?;
    let invalid = match ip {
        IpAddr::V4(v) => {
            v.is_private()
                || v.is_loopback()
                || v.is_link_local()
                || v.is_unspecified()
                || v.is_multicast()
                || v.is_broadcast()
        }
        IpAddr::V6(v) => {
            v.is_loopback()
                || v.is_unspecified()
                || v.is_multicast()
                || (v.segments()[0] & 0xfe00) == 0xfc00
                || (v.segments()[0] & 0xffc0) == 0xfe80
        }
    };
    if invalid {
        return Err("IP 服务未返回公网地址".into());
    }
    Ok((
        ip.to_string(),
        if ip.is_ipv4() { "IPv4" } else { "IPv6" }.into(),
    ))
}
fn geo(raw: &str) -> Result<IpInfo, String> {
    let data: Value = serde_json::from_str(raw).map_err(|_| "IP 归属信息无法读取")?;
    if data["success"] != true {
        return Err("IP 归属服务未完成查询".into());
    }
    let (ip, version) = address(data["ip"].as_str().ok_or("IP 地址缺失")?)?;
    Ok(IpInfo {
        ip,
        version,
        country: text(&data["country"]),
        region: text(&data["region"]),
        city: text(&data["city"]),
        isp: text(&data["connection"]["isp"]),
        organization: text(&data["connection"]["org"]),
        asn: data["connection"]["asn"]
            .as_u64()
            .and_then(|n| u32::try_from(n).ok()),
        domain: text(&data["connection"]["domain"]),
        source: "ipwho.is",
        checked_at: chrono::Utc::now().to_rfc3339(),
        warning: None,
    })
}
fn trace(raw: &str) -> Result<IpInfo, String> {
    let field = |key: &str| {
        raw.lines()
            .find_map(|line| line.strip_prefix(&format!("{key}=")))
    };
    let (ip, version) = address(field("ip").ok_or("公网 IP 信息缺失")?)?;
    Ok(IpInfo {
        ip,
        version,
        country: field("loc").map(|s| s.chars().take(2).collect()),
        region: None,
        city: None,
        isp: None,
        organization: None,
        asn: None,
        domain: None,
        source: "Cloudflare",
        checked_at: chrono::Utc::now().to_rfc3339(),
        warning: Some("归属服务暂不可用，已获取公网 IP；服务商和城市未核实。".into()),
    })
}
fn script(body: &str) -> String {
    format!("$ErrorActionPreference='Stop';Add-Type -TypeDefinition @'\n{ENGINE}\n'@ -ReferencedAssemblies 'System.Net.Http','System','System.Core';\n{body}")
}
fn fetch(url: &str) -> Result<String, String> {
    super::runtime::run(
        &super::root()?.join("Insights"),
        &script(&format!("[WinEaseInsights]::Fetch('{url}')")),
        20,
    )
}
#[tauri::command]
pub async fn network_site_tests() -> Result<Vec<Site>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let _guard = SITES.try_lock().map_err(|_| "网站测试正在进行")?;
        let raw = super::runtime::run(
            &super::root()?.join("Insights"),
            &script("ConvertTo-Json -InputObject @([WinEaseInsights]::Sites()) -Depth 4 -Compress"),
            25,
        )?;
        serde_json::from_str(&raw).map_err(|_| "网站测试结果无法读取".into())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn network_ip_info() -> Result<IpInfo, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let _guard = IP.try_lock().map_err(|_| "IP 查询正在进行")?;
        match fetch(GEO).and_then(|raw| geo(&raw)) {
            Ok(info) => Ok(info),
            Err(_) => fetch(TRACE)
                .and_then(|raw| trace(&raw))
                .map_err(|_| "IP 服务暂不可达，请稍后重试。".into()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn geo_preserves_missing_fields_and_rejects_false_success() {
        let row=geo(r#"{"success":true,"ip":"8.8.8.8","connection":{"asn":15169,"isp":"Google","org":"Google LLC"},"city":"Mountain View"}"#).unwrap();
        assert_eq!(row.asn, Some(15169));
        assert_eq!(row.version, "IPv4");
        assert!(row.country.is_none());
        assert!(geo(r#"{"success":false,"ip":"8.8.8.8"}"#).is_err());
        for ip in ["127.0.0.1", "192.168.1.1", "::1", "fc00::1", "not-an-ip"] {
            assert!(address(ip).is_err());
        }
        assert_eq!(
            trace("ip=2001:4860:4860::8888\nloc=US\n").unwrap().version,
            "IPv6"
        );
        assert!(trace("ip=8.8.8.8\nloc=US").unwrap().isp.is_none());
    }
    #[test]
    #[ignore = "explicit HTTPS website headers and IP metadata lookup; preserves proxy; no system changes"]
    fn current_connection_insights_are_readable() {
        let sites = tauri::async_runtime::block_on(network_site_tests()).unwrap();
        assert_eq!(sites.len(), 6);
        for site in sites {
            println!(
                "{}: reachable={} status={:?}",
                site.name, site.reachable, site.status_code
            );
        }
        let ip = tauri::async_runtime::block_on(network_ip_info()).unwrap();
        assert!(address(&ip.ip).is_ok());
        println!("IP metadata readable: {} from {}", ip.version, ip.source);
    }
}

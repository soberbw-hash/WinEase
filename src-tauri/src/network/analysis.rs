use super::{arr, Issue, Probe, Scan, Tool};
use serde_json::Value;

#[derive(serde::Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Endpoint {
    pub host: String,
    pub port: u16,
    pub scheme: String,
}

pub fn endpoints(server: &str) -> Option<Vec<Endpoint>> {
    if server.is_empty() || server.len() > 2048 {
        return None;
    }
    let mut result = Vec::new();
    for part in server.split(';').filter(|p| !p.trim().is_empty()) {
        let (scheme, address) = part.trim().split_once('=').unwrap_or(("http", part.trim()));
        if !["http", "https", "socks"].contains(&scheme.to_ascii_lowercase().as_str()) {
            return None;
        }
        let address = address
            .strip_prefix("http://")
            .or_else(|| address.strip_prefix("https://"))
            .unwrap_or(address);
        let (host, port) = address.rsplit_once(':')?;
        let host = host.trim_matches(['[', ']']).to_ascii_lowercase();
        if !["127.0.0.1", "localhost", "::1"].contains(&host.as_str()) {
            return None;
        }
        let port: u16 = port.parse().ok()?;
        if port == 0 {
            return None;
        }
        let endpoint = Endpoint {
            host,
            port,
            scheme: scheme.to_ascii_lowercase(),
        };
        if !result.iter().any(|e: &Endpoint| {
            e.host == endpoint.host && e.port == port && e.scheme == endpoint.scheme
        }) {
            result.push(endpoint);
        }
        if result.len() > 8 {
            return None;
        }
    }
    (!result.is_empty()).then_some(result)
}
pub fn tunnel(adapter: &Value) -> bool {
    if adapter["hardwareInterface"].as_bool().unwrap_or(true) {
        return false;
    }
    let label = format!(
        "{} {}",
        adapter["name"].as_str().unwrap_or(""),
        adapter["description"].as_str().unwrap_or("")
    )
    .to_ascii_lowercase();
    [
        "tun",
        "tap",
        "wintun",
        "clash",
        "mihomo",
        "xsus",
        "sing-box",
        "wireguard",
        "openvpn",
        "tailscale",
    ]
    .iter()
    .any(|s| label.contains(s))
}
pub fn orphan_routes(raw: &Value) -> Vec<Value> {
    arr(raw, "routes")
        .iter()
        .filter(|route| {
            arr(raw, "adapters").iter().any(|a| {
                a["interfaceIndex"] == route["interfaceIndex"]
                    && tunnel(a)
                    && ["Disconnected", "Not Present"].contains(&a["status"].as_str().unwrap_or(""))
            })
        })
        .cloned()
        .collect()
}
pub fn dead_proxy(raw: &Value, probes: &[Probe]) -> bool {
    if raw["proxy"]["enabled"] != true
        || !raw["proxy"]["autoConfigUrl"]
            .as_str()
            .unwrap_or("")
            .is_empty()
        || raw["managed"] == true
    {
        return false;
    }
    let Some(eps) = endpoints(raw["proxy"]["server"].as_str().unwrap_or("")) else {
        return false;
    };
    eps.iter().all(|e| {
        probes
            .iter()
            .any(|p| p.id == format!("local-{}-{}", e.port, e.host) && !p.ok)
    })
}
pub fn analyze(raw: Value, probes: Vec<Probe>, tools: &[Tool]) -> Scan {
    let mut issues = Vec::new();
    let mut actions = Vec::new();
    let mut add = |id: &str, title: &str, detail: &str, action: Option<&str>| {
        issues.push(Issue {
            id: id.into(),
            title: title.into(),
            detail: detail.into(),
            repairable: action.is_some(),
        });
        if let Some(action) = action {
            if !actions.iter().any(|a| a == action) {
                actions.push(action.to_string());
            }
        }
    };
    let up: Vec<_> = arr(&raw, "adapters")
        .iter()
        .filter(|a| a["status"] == "Up")
        .collect();
    let connected = up.iter().any(|a| a["hardwareInterface"] == true);
    let system_ok = probes.iter().any(|p| p.id.starts_with("system-") && p.ok);
    let portal = probes.iter().any(|p| p.portal);
    let dns_ok = arr(&raw, "dnsTests").iter().any(|d| d["ok"] == true);
    let collection_ok = arr(&raw, "collectionErrors").is_empty();
    if !collection_ok {
        add(
            "incomplete",
            "部分配置未能读取",
            "打开检测详情查看原因；当前结果不能代表完整网络状态。",
            None,
        );
    }
    if up.is_empty() {
        add(
            "disconnected",
            "没有已连接的网卡",
            "检查网线、Wi-Fi 或飞行模式；软件无法接通断开的网络。",
            None,
        );
    }
    if portal && !system_ok {
        add(
            "portal",
            "网络可能需要登录",
            "打开浏览器完成酒店、校园或公共 Wi-Fi 的联网验证。",
            None,
        );
    }
    if dead_proxy(&raw, &probes) {
        add(
            "dead-proxy",
            "系统代理已失效",
            "代理指向的本机端口均无法连接，修复将关闭这条失效代理。",
            collection_ok.then_some("disable-dead-proxy"),
        );
    }
    if !orphan_routes(&raw).is_empty() {
        add(
            "orphan-routes",
            "发现断开隧道的残留路由",
            "只清理已断开的 TUN 默认/分流路由，保留活动隧道。",
            collection_ok.then_some("remove-orphan-tun-routes"),
        );
    }
    if !dns_ok && connected && !portal {
        add(
            "dns",
            "域名解析未通过",
            "先刷新 DNS 缓存；保留现有 DNS 服务器。",
            collection_ok.then_some("flush-dns"),
        );
    }
    if connected && !system_ok && !portal && !up.iter().any(|a| tunnel(a)) && raw["managed"] != true
    {
        let apipa = arr(&raw, "ips").iter().any(|ip| {
            ip["dhcp"] == true
                && up.iter().any(|a| {
                    a["hardwareInterface"] == true && a["interfaceIndex"] == ip["interfaceIndex"]
                })
                && arr(ip, "addresses")
                    .iter()
                    .any(|v| v.as_str().is_some_and(|s| s.starts_with("169.254.")))
        });
        if apipa {
            add(
                "dhcp",
                "未获取有效的网络地址",
                "为出现自动私有地址的 DHCP 物理网卡重新申请地址，期间可能短暂断网。",
                collection_ok.then_some("renew-dhcp"),
            );
        }
    }
    if !system_ok && !portal && !up.is_empty() {
        add(
            "egress",
            "系统连接探测未通过",
            "可能是上游网络、代理节点或站点限制；自动修复不会重置防火墙和活动代理。",
            None,
        );
    }
    if system_ok
        && probes
            .iter()
            .filter(|p| p.id.starts_with("direct-"))
            .all(|p| !p.ok)
    {
        add(
            "proxy-route",
            "当前连接依赖代理或隧道",
            "系统连接可用，保留正在使用的代理和 TUN。",
            None,
        );
    }
    if probes
        .iter()
        .any(|p| p.id == "proxy-egress" && !p.ok && !p.reachable)
    {
        add(
            "proxy-egress",
            "代理出口验证未通过",
            "本地端口可用不代表节点出口可用；请在代理软件中检查节点。",
            None,
        );
    }
    if probes
        .iter()
        .any(|p| p.id == "proxy-egress" && !p.ok && p.reachable)
    {
        add(
            "site-restriction",
            "代理已连接，测试站点拒绝请求",
            "站点响应不代表服务可正常使用；此结果不触发代理重置。",
            None,
        );
    }
    let mut families = std::collections::HashSet::new();
    for process in arr(&raw, "proxyProcesses") {
        let name = process["name"].as_str().unwrap_or("").to_ascii_lowercase();
        families.insert(if name.contains("clash") || name.contains("mihomo") {
            "clash".to_string()
        } else {
            name
        });
    }
    if families.len() > 1 {
        add(
            "proxy-programs",
            "检测到多个代理或隧道程序",
            "同时运行不等于冲突，保留现有连接；连接不稳定时在原软件中检查。",
            None,
        );
    }
    if raw["managed"] == true {
        add(
            "managed",
            "检测到受管理的网络配置",
            "自动修复保留单位网络的代理和 DNS 设置。",
            None,
        );
    }
    let plan = actions
        .iter()
        .filter_map(|id| tools.iter().find(|t| &t.id == id).cloned())
        .collect();
    Scan {
        id: uuid::Uuid::new_v4().to_string(),
        checked_at: chrono::Utc::now().to_rfc3339(),
        state: if !collection_ok {
            "unknown"
        } else if !actions.is_empty() {
            "repairable"
        } else if system_ok {
            "healthy"
        } else {
            "attention"
        }
        .into(),
        summary: if !collection_ok {
            "检查未完整完成"
        } else if !actions.is_empty() {
            "发现可修复的问题"
        } else if system_ok {
            "基本联网可用"
        } else {
            "网络需要进一步检查"
        }
        .into(),
        issues,
        plan,
        raw,
        probes,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn base() -> Value {
        json!({"adapters":[{"status":"Up","hardwareInterface":true}],"dnsTests":[{"ok":true}],"proxy":{"enabled":true,"server":"127.0.0.1:7897"},"routes":[],"collectionErrors":[]})
    }
    fn p(id: &str, ok: bool) -> Probe {
        Probe {
            id: id.into(),
            name: "test".into(),
            target: "test".into(),
            ok,
            reachable: ok,
            portal: false,
            latency_ms: 0,
            detail: String::new(),
        }
    }
    #[test]
    fn validates_every_endpoint_and_does_not_treat_socks_as_http() {
        assert_eq!(endpoints("socks=[::1]:1080").unwrap()[0].scheme, "socks");
        for s in [
            "127.0.0.1:70000",
            "localhost:0",
            "http=127.0.0.1:7890;https=company:8080",
            "127.0.0.1:7897&whoami",
        ] {
            assert!(endpoints(s).is_none());
        }
    }
    #[test]
    fn working_proxy_is_never_repaired() {
        let scan = analyze(
            base(),
            vec![p("local-7897-127.0.0.1", true), p("system-cn", true)],
            &super::super::tools(),
        );
        assert!(scan.plan.is_empty());
        assert_eq!(scan.state, "healthy");
    }
    #[test]
    fn dead_proxy_requires_all_endpoints_failed_and_no_pac_or_policy() {
        let mut raw = base();
        raw["proxy"]["server"] = json!("http=127.0.0.1:7897;https=localhost:7898");
        let probes = vec![
            p("local-7897-127.0.0.1", false),
            p("local-7898-localhost", true),
        ];
        assert!(!dead_proxy(&raw, &probes));
        let probes = vec![
            p("local-7897-127.0.0.1", false),
            p("local-7898-localhost", false),
        ];
        assert!(dead_proxy(&raw, &probes));
        raw["managed"] = json!(true);
        assert!(!dead_proxy(&raw, &probes));
        raw["managed"] = json!(false);
        raw["proxy"]["autoConfigUrl"] = json!("https://example/pac");
        assert!(!dead_proxy(&raw, &probes));
    }
    #[test]
    fn missing_adapter_is_not_a_route_deletion_candidate() {
        let raw = json!({"adapters":[],"routes":[{"interfaceIndex":9}]});
        assert!(orphan_routes(&raw).is_empty());
    }
    #[test]
    fn collection_failure_disables_automatic_mutations() {
        let mut raw = base();
        raw["collectionErrors"] = json!(["adapter error"]);
        let s = analyze(
            raw,
            vec![p("local-7897-127.0.0.1", false)],
            &super::super::tools(),
        );
        assert!(s.plan.is_empty());
        assert_eq!(s.state, "unknown");
    }
    #[test]
    fn healthy_dns_and_one_failed_site_do_not_trigger_reset() {
        let s = analyze(
            base(),
            vec![
                p("system-cn", true),
                p("system-ms", false),
                p("local-7897-127.0.0.1", true),
            ],
            &super::super::tools(),
        );
        assert!(s.plan.is_empty());
    }
}

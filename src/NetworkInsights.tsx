import { useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { CheckIcon } from "./CheckIcon";
type Site = {id:string; name:string; url:string; area:string; reachable:boolean; ok:boolean; statusCode:number|null; latencyMs:number|null; error:string|null};
type Ip = {ip:string; version:string; country:string|null; region:string|null; city:string|null; isp:string|null; organization:string|null; asn:number|null; domain:string|null; source:string; checkedAt:string; warning:string|null};
export function NetworkInsights({view}:{view:string}) {
  const [sites,setSites]=useState<Site[]>([]),[ip,setIp]=useState<Ip|null>(null);
  const [sitesBusy,setSitesBusy]=useState(false),[ipBusy,setIpBusy]=useState(false);
  const [sitesError,setSitesError]=useState(""),[ipError,setIpError]=useState("");
  const [copyState,setCopyState]=useState("");
  const [sitesCheckedAt,setSitesCheckedAt]=useState("");
  const locks=useRef(new Set<string>());
  async function run(kind:"sites"|"ip") {
    if(locks.current.has(kind))return;
    locks.current.add(kind);
    if(kind==="sites"){setSitesBusy(true);setSitesError("");}else{setIpBusy(true);setIpError("");setCopyState("");}
    try {
      if(kind==="sites"){setSites(await invoke<Site[]>("network_site_tests"));setSitesCheckedAt(new Date().toLocaleString("zh-CN",{hour12:false}));}
      else setIp(await invoke<Ip>("network_ip_info"));
    }catch(e){if(kind==="sites")setSitesError(String(e));else setIpError(String(e));}
    finally{locks.current.delete(kind);if(kind==="sites")setSitesBusy(false);else setIpBusy(false);}
  }
  async function copyIp(){try{if(ip){await navigator.clipboard.writeText(ip.ip);setCopyState("已复制");}}catch{setCopyState("复制失败");}}
  return <>
    <section className="surface" hidden={view!=="sites"} aria-label="网站测试">
      <div className="section-head"><h2>网站连通测试</h2><button className="primary-button" disabled={sitesBusy||!isTauri()} onClick={()=>void run("sites")}>{sitesBusy?"测试中…":sites.length?"重新测试":"开始测试"}</button></div>
      <p className="scope-note">百度、哔哩哔哩、腾讯、GitHub、Google、YouTube · 沿用当前代理，测试访问状态和响应时间。</p>
      {sitesError&&<p className="inline-error" role="alert">{sitesError}</p>}
      {!sites.length&&<p className="scope-note">点击开始测试，查看国内外网站是否可达。</p>}
      <div className="network-sites-grid">{sites.map(site=><article className="soft-card" key={site.id}>
        <div className="section-head"><h3><CheckIcon kind="network"/>{site.name}</h3><span className={`pill ${site.ok?"pill--success":site.reachable?"pill--warning":"pill--muted"}`}>{site.ok?"可访问":site.reachable?"站点限制":"未连接"}</span></div>
        <small>{site.area} · {site.url.replace("https://","")}</small>
        <p>{site.latencyMs!=null?`${site.latencyMs} ms · `:""}{site.statusCode!=null?`HTTP ${site.statusCode}`:site.error||"未核实"}</p>
      </article>)}</div>
      {sites.length>0&&<p className="scope-note">上次测试 {sitesCheckedAt} · 响应时间包含连接与服务器响应，并非下载速度。403、429 等表示收到网站响应，不等于断网。</p>}
    </section>
    <section className="surface" hidden={view!=="ip"} aria-label="IP 信息">
      <div className="section-head"><h2>公网 IP 信息</h2><button className="primary-button" disabled={ipBusy||!isTauri()} onClick={()=>void run("ip")}>{ipBusy?"查询中…":ip?"刷新信息":"查询 IP"}</button></div>
      <p className="scope-note">显示当前连接的出口 IP；使用代理时可能是代理出口。位置是 IP 数据库的估计，不是设备的精确地址。</p>
      {ipError&&<p className="inline-error" role="alert">{ipError}</p>}
      {ip&&<>
        <div className="network-public-ip"><div><small>{ip.version}</small><strong>{ip.ip}</strong></div><button className="ghost-button" onClick={()=>void copyIp()}>{copyState||"复制 IP"}</button></div>
        <dl className="network-ip-grid">{[["服务商",ip.isp],["组织",ip.organization],["国家",ip.country],["地区",ip.region],["城市",ip.city],["自治系统",ip.asn!=null?`AS${ip.asn}`:null],["组织域名",ip.domain]].map(([label,value])=><div key={label}><dt>{label}</dt><dd>{value||"未提供"}</dd></div>)}</dl>
        {ip.warning&&<p className="scope-note" role="status">{ip.warning}</p>}
        <p className="scope-note">来源 {ip.source} · {new Date(ip.checkedAt).toLocaleString("zh-CN",{hour12:false})}</p>
      </>}
    </section>
  </>;
}

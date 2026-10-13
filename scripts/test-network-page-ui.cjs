async (page) => {
  await page.addInitScript(()=>{
    window.isTauri=true;window.calls=[];let cb=0;
    window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:()=>{}};
    const scan={id:'scan',checkedAt:new Date().toISOString(),state:'healthy',summary:'网络连接正常',issues:[],plan:[],probes:[],raw:{adapters:[{interfaceIndex:1,name:'Ethernet',description:'fixture',linkSpeed:'1 Gbps',status:'Up',hardwareInterface:true}],dnsTests:[{ok:true}],proxy:{enabled:false},dns:[],ips:[],proxyProcesses:[],proxyServices:[],routes:[],hostsEntries:[],collectionErrors:[],environmentProxy:[]}};
    window.__TAURI_INTERNALS__={transformCallback:()=>++cb,unregisterCallback:()=>{},invoke:async(cmd,args)=>{
      window.calls.push({cmd,args});
      if(cmd==='network_scan')return scan;
      if(['network_tools','network_backups','list_components','check_component_updates','storage_drives','get_optimization_presets'].includes(cmd))return [];
      if(cmd==='network_site_tests')return new Promise(resolve=>window.finishSites=()=>resolve([{id:'github',name:'GitHub',url:'https://github.com/',area:'国外',ok:true,reachable:true,statusCode:200,latencyMs:120,error:null},{id:'bilibili',name:'哔哩哔哩',url:'https://www.bilibili.com/',area:'国内',ok:false,reachable:true,statusCode:403,latencyMs:50,error:null},{id:'google',name:'Google',url:'https://www.google.com/',area:'国外',ok:false,reachable:false,statusCode:null,latencyMs:null,error:'连接超时'}]));
      if(cmd==='network_ip_info')return {ip:'8.8.8.8',version:'IPv4',country:'United States',region:'California',city:'Mountain View',isp:'Google',organization:'Google LLC',asn:15169,domain:'google.com',source:'fixture',checkedAt:new Date().toISOString(),warning:null};
      if(cmd==='network_speedtest')return new Promise(resolve=>window.finishSpeed=()=>resolve({latency:{latencyMs:21,jitterMs:3,error:null},download:{mbps:120,error:null,requestedBytes:100},upload:{mbps:30,error:null,requestedBytes:50},checkedAt:new Date().toISOString(),server:'fixture'}));
      return null;
    }};
  });
  await page.goto('http://127.0.0.1:1420');await page.setViewportSize({width:1280,height:820});
  const nav=page.getByRole('navigation',{name:'主导航'});
  if(await page.evaluate(()=>window.calls.some(c=>c.cmd.startsWith('network_'))))throw Error('Hidden network tools run on homepage');
  await nav.getByRole('button',{name:'网络',exact:true}).click();
  await page.getByRole('heading',{name:'网络连接正常',exact:true}).waitFor();
  const tabs=page.getByRole('navigation',{name:'网络分类'});
  if(await page.getByRole('button',{name:'开始测速',exact:true}).isVisible())throw Error('Bandwidth mixed into diagnostics');
  await tabs.getByRole('button',{name:'网站测试',exact:true}).click();
  await page.getByRole('button',{name:'开始测试',exact:true}).click();
  await tabs.getByRole('button',{name:'IP 信息',exact:true}).click();
  await page.getByRole('button',{name:'查询 IP',exact:true}).click();await page.getByText('AS15169',{exact:true}).waitFor();
  if(await page.getByRole('button',{name:'刷新信息',exact:true}).isDisabled())throw Error('Site task blocks IP task');
  await nav.getByRole('button',{name:'一键优化',exact:true}).click();await nav.getByRole('button',{name:'网络',exact:true}).click();
  await page.getByText('AS15169',{exact:true}).waitFor();
  if(await page.evaluate(()=>window.calls.filter(c=>c.cmd==='network_scan').length)!==1)throw Error('Network navigation loses state/restarts scan');
  await tabs.getByRole('button',{name:'网站测试',exact:true}).click();await page.evaluate(()=>window.finishSites());
  await page.getByText('站点限制',{exact:true}).waitFor();await page.getByText('未连接',{exact:true}).waitFor();
  await page.screenshot({path:'output/playwright/network-sites.png'});
  await tabs.getByRole('button',{name:'网速测试',exact:true}).click();await page.getByRole('button',{name:'开始测速',exact:true}).click();
  await page.getByRole('button',{name:'停止测速',exact:true}).waitFor();
  await tabs.getByRole('button',{name:'IP 信息',exact:true}).click();await page.getByRole('button',{name:'刷新信息',exact:true}).click();
  await page.evaluate(()=>window.finishSpeed());
  await page.screenshot({path:'output/playwright/network-ip.png'});
  await tabs.getByRole('button',{name:'网速测试',exact:true}).click();await page.getByRole('button',{name:'重新测速',exact:true}).waitFor();
  for(const width of [1280,760]){await page.setViewportSize({width,height:820});if(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth))throw Error('Network horizontal overflow');}
  await nav.getByRole('button',{name:'系统工具',exact:true}).click();
  await page.getByRole('button',{name:/刷新软件安装列表/}).waitFor();
  if(await page.locator('summary').getByText('按问题修复与恢复',{exact:true}).count())throw Error('Maintenance still collapsed');
  return {passed:['independent network sidebar','no hidden network operations','diagnostics distinct from speed','site/IP tasks independent','IP and pending results persist across navigation','HTTP restriction distinct from connection error','manual bandwidth task and IP independent','responsive layout','maintenance tools directly visible']};
}

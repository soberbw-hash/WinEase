async (page) => {
  await page.addInitScript(() => {
    window.isTauri = true; window.calls = []; let cb = 0;
    let apps = [
      {name:'Microsoft.BingWeather',fullName:'weather_1',family:'weather_family',label:'Microsoft 天气',purpose:'天气应用',version:'1',dependencies:[],sizeBytes:1000000,sizeLimited:false,removable:true,removed:false,iconTarget:'C:\\Apps\\weather.exe'},
      {name:'Microsoft.WindowsStore',fullName:'store_1',family:'store_family',label:'Microsoft Store',purpose:'系统组件',version:'1',dependencies:[],sizeBytes:null,sizeLimited:true,removable:false,removed:false,iconTarget:null},
    ];
    const item = (id,title,extra={}) => ({id,title,detail:'夹具证据',status:'optional',sizeBytes:0,fileCount:0,selectable:true,defaultSelected:false,actionLabel:null,target:null,iconTarget:null,iconKind:'system',hasFiles:false,...extra});
    window.report={id:'community-report',revision:1,phase:'ready',checkedAt:new Date().toISOString(),outcomes:[],completedActions:0,totalActions:0,sections:[
      {id:'cleaning',title:'深度清理',status:'complete',summary:'缓存',items:[item('cache','应用缓存',{status:'recommended',defaultSelected:true,actionLabel:'清理缓存'})]},
      ...['files','health','network','startup'].map(id=>({id,title:id,status:'complete',summary:'检查完成',items:[]})),
      {id:'performance',title:'启动与后台分析',status:'complete',summary:'6 秒采样',items:[item('cpu','chrome',{selectable:false,detail:'2 个进程，平均 CPU 18%',target:'open_processes'})]},
      {id:'maintenance',title:'更新与应用安装',status:'complete',summary:'服务',items:[item('maint','更新服务异常',{actionLabel:'修复安装与更新'})]},
      {id:'preferences',title:'功能偏好',status:'complete',summary:'保留偏好',items:[item('pref','减少系统使用建议',{actionLabel:'调整功能偏好'})]},
      {id:'apps',title:'预装应用',status:'complete',summary:'默认保留',items:[item('app','Microsoft 天气',{actionLabel:'卸载可选应用'})]},
    ]};
    window.__TAURI_INTERNALS__={transformCallback:()=>++cb,unregisterCallback:()=>{},invoke:async(cmd,args)=>{
      window.calls.push({cmd,args});
      if(['list_components','storage_drives','check_component_updates','list_processes','list_startup_items'].includes(cmd)) return [];
      if(['get_application_icon','get_file_icon','plugin:updater|check'].includes(cmd)) return null;
      if(cmd==='get_windows_settings')return [];
      if(cmd==='get_power_plan')return {mode:'balanced',performanceAvailable:true};
      if(cmd==='analyze_resources')return new Promise(resolve=>window.finishResources=()=>resolve({seconds:6,groups:[{name:'chrome',path:'C:\\Apps\\chrome.exe',cpuPercent:18,memoryBytes:200000000,processCount:2,attention:true}],boot:null,bootError:'权限不足',startup:[]}));
      if(cmd==='list_preinstalled_apps')return structuredClone(apps);
      if(cmd==='manage_preinstalled_app'){if(!args.confirmed)throw '未确认';apps=apps.map(a=>a.name===args.name?{...a,removed:!args.restore,removable:args.restore}:a);return args.restore?'应用已重新注册；原应用数据不保证恢复。':'已从当前用户移除';}
      if(cmd==='check_maintenance')return {pendingReboot:false,checks:[{id:'winget',title:'WinGet 命令',status:'attention',detail:'注册异常',repair:'winget-register'}]};
      if(cmd==='repair_maintenance')return new Promise(resolve=>window.finishMaintenance=()=>resolve('已重新注册并核对包状态'));
      if(cmd==='start_optimization_check')return structuredClone(window.report);
      if(cmd==='optimization_status')return args.revision===window.report.revision?null:structuredClone(window.report);
      if(cmd==='start_optimization'){window.report={...window.report,phase:'complete',revision:2};return structuredClone(window.report);}
      return null;
    }};
  });
  await page.goto('http://127.0.0.1:1420');await page.setViewportSize({width:1280,height:820});
  const nav=page.getByRole('navigation',{name:'主导航'});
  await nav.getByRole('button',{name:'应用管理',exact:true}).click();
  await page.getByRole('button',{name:'开始分析',exact:true}).click();
  await page.getByRole('button',{name:'预装应用',exact:true}).click();
  await page.getByRole('button',{name:'扫描应用',exact:true}).click();
  await page.getByText('Microsoft 天气',{exact:true}).waitFor();
  if(await page.getByRole('button',{name:'受保护',exact:true}).count())throw Error('Protected packages crowd default list');
  await page.getByRole('switch',{name:'显示受保护应用',exact:true}).check();
  if(!await page.getByRole('button',{name:'受保护',exact:true}).isDisabled())throw Error('Critical package removable');
  await page.getByRole('button',{name:'卸载',exact:true}).click();
  const dialog=page.getByRole('alertdialog');await dialog.waitFor();
  if(!await dialog.textContent().then(t=>t.includes('可能删除应用数据')))throw Error('Missing app data boundary');
  if(await page.evaluate(()=>window.calls.some(c=>c.cmd==='manage_preinstalled_app')))throw Error('Uninstall before consent');
  await page.getByRole('button',{name:'取消',exact:true}).click();
  await page.getByRole('button',{name:'卸载',exact:true}).click();await page.getByRole('button',{name:'确认卸载',exact:true}).click();
  await page.getByRole('button',{name:'尝试恢复',exact:true}).waitFor();
  await page.getByRole('button',{name:'尝试恢复',exact:true}).click();
  await dialog.getByRole('button',{name:'尝试恢复',exact:true}).click();
  await page.getByRole('button',{name:'卸载',exact:true}).waitFor();
  if(!await page.evaluate(()=>window.calls.some(c=>c.cmd==='manage_preinstalled_app'&&c.args.restore&&c.args.confirmed)))throw Error('Restore not routed');
  await page.evaluate(()=>window.finishResources());
  await nav.getByRole('button',{name:'系统工具',exact:true}).click();
  await page.getByRole('button',{name:'检查状态',exact:true}).click();
  await page.getByText('注册异常',{exact:true}).waitFor();
  const maintenance=page.locator('section.surface').filter({has:page.getByRole('heading',{name:'更新与应用安装',exact:true})});
  await maintenance.getByRole('button',{name:'修复',exact:true}).click();
  await dialog.getByRole('button',{name:'确认执行',exact:true}).click();
  await nav.getByRole('button',{name:'应用管理',exact:true}).click();
  await page.getByRole('button',{name:'进程管理',exact:true}).click();
  await page.getByText('CPU 18.0% · 190.7 MB',{exact:true}).waitFor();
  if(await page.evaluate(()=>window.calls.filter(c=>c.cmd==='analyze_resources').length)!==1)throw Error('Cross-page navigation lost resource analysis');
  await page.evaluate(()=>window.finishMaintenance());
  await nav.getByRole('button',{name:'系统工具',exact:true}).click();
  await page.getByText('已重新注册并核对包状态',{exact:true}).waitFor();
  await nav.getByRole('button',{name:'一键优化',exact:true}).click();
  await page.getByRole('button',{name:'一键检查',exact:true}).click();
  for(const name of ['选择 Microsoft 天气','选择 减少系统使用建议','选择 更新服务异常'])if(await page.getByRole('checkbox',{name,exact:true}).isChecked())throw Error('Optional action selected automatically: '+name);
  await page.getByRole('checkbox',{name:'选择 Microsoft 天气',exact:true}).check();
  await page.getByRole('checkbox',{name:'选择 减少系统使用建议',exact:true}).check();
  await page.getByRole('region',{name:'检查概览'}).getByRole('button',{name:'一键优化',exact:true}).click();
  if(!await dialog.textContent().then(t=>t.includes('可能删除应用数据')&&t.includes('原值')))throw Error('Unified consent omitted new actions');
  await dialog.getByRole('button',{name:'确认并优化',exact:true}).click();
  if(!await page.evaluate(()=>window.calls.some(c=>c.cmd==='start_optimization'&&c.args.itemIds.includes('app')&&c.args.itemIds.includes('pref')&&c.args.confirmed)))throw Error('Unified plan lost selection');
  for(const width of [1280,760]) {
    await page.setViewportSize({width,height:820});
    await nav.getByRole('button',{name:'应用管理',exact:true}).click();
    await page.getByRole('button',{name:'预装应用',exact:true}).click();
    if(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth))throw Error('Community layout overflows '+width);
  }
  await page.setViewportSize({width:1280,height:820});
  await page.screenshot({path:'output/playwright/community-apps.png',fullPage:true});
  console.log('PASS: 10 community flows: independent sampling, protected apps, removal consent, restore, cross-page evidence, independent maintenance, retained repair result, optional defaults, unified consent/actions, responsive layout');
}

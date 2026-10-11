async (page) => {
  await page.addInitScript(() => {
    window.isTauri=true;window.calls=[];let callback=0;
    let settings=[{id:'hidden-files',label:'显示隐藏文件',description:'隐藏文件',enabled:false,canRestore:false,error:null},{id:'block-suggested-apps',label:'阻止自动安装推荐应用',description:'保留已有应用',enabled:false,canRestore:false,error:null},{id:'reduce-explorer-promotions',label:'减少资源管理器推广',description:'推广提示',enabled:true,canRestore:false,error:null},{id:'autoplay',label:'移动设备自动播放',description:'移动设备',enabled:null,canRestore:false,error:null}];
    window.__TAURI_INTERNALS__={transformCallback:()=>++callback,unregisterCallback:()=>{},invoke:async(cmd,args)=>{
      window.calls.push({cmd,args});
      if(cmd==='storage_drives')return [];
      if(cmd==='get_windows_settings')return settings.map(s=>({...s}));
      if(cmd==='get_power_plan')throw '电源计划读取失败';
      if(cmd==='set_windows_setting'){settings=settings.map(s=>s.id===args.id?{...s,enabled:args.enabled,canRestore:true}:s);return null;}
      if(cmd==='restore_windows_setting'){settings=settings.map(s=>s.id===args.id?{...s,enabled:null,canRestore:false}:s);return null;}
      if(cmd==='repair_windows')return new Promise(resolve=>window.finishRepair=()=>resolve({success:true,message:'夹具修复完成',logs:'fixture only',restart:false}));
      if(cmd==='list_components'||cmd==='check_component_updates')return [];
      if(cmd==='get_application_icon'||cmd==='plugin:updater|check')return null;
      return null;
    }};
  });
  await page.goto('http://127.0.0.1:1420');await page.setViewportSize({width:1280,height:820});
  const nav=page.getByRole('navigation',{name:'主导航'});
  if(await page.evaluate(()=>window.calls.some(c=>c.cmd==='get_windows_settings')))throw Error('Hidden system page probes at startup');
  await nav.getByRole('button',{name:'系统工具',exact:true}).click();
  await page.getByRole('switch',{name:'减少资源管理器推广',exact:true}).waitFor();
  if(await page.getByRole('switch',{name:'透明效果',exact:true}).count() || await page.getByRole('button',{name:/视觉效果|任务栏设置/}).count())throw Error('Beautification controls still present');
  await page.getByRole('switch',{name:'阻止自动安装推荐应用',exact:true}).check();
  if(!await page.evaluate(()=>window.calls.some(c=>c.cmd==='set_windows_setting'&&c.args.id==='block-suggested-apps'&&c.args.enabled===true)))throw Error('Suggested-app block not applied');
  await page.getByText('电源计划读取失败',{exact:true}).waitFor();
  await page.getByRole('combobox',{name:'移动设备自动播放',exact:true}).selectOption('off');
  await page.getByRole('switch',{name:'移动设备自动播放',exact:true}).waitFor();
  if(await page.getByRole('switch',{name:'移动设备自动播放'}).isChecked())throw Error('Cannot disable unset preference');
  const row=page.locator('.setting-line').filter({has:page.getByRole('switch',{name:'移动设备自动播放'})});
  await row.getByRole('button',{name:'恢复原设置',exact:true}).click();
  await page.getByRole('combobox',{name:'移动设备自动播放'}).waitFor();
  await page.getByRole('button',{name:'磁盘优化',exact:false}).click();
  if(!await page.evaluate(()=>window.calls.some(c=>c.cmd==='open_health_tool'&&c.args.tool==='open_drive_optimizer')))throw Error('Drive optimizer routed incorrectly');
  if(await page.getByRole('button',{name:/默认应用/}).count()!==1)throw Error('Duplicate default-app entry');
  await page.getByRole('button',{name:'检查并修复',exact:true}).click();
  await page.getByRole('button',{name:'修复中…',exact:true}).waitFor();
  if(await page.getByRole('switch',{name:'减少资源管理器推广'}).isDisabled())throw Error('System repair blocks unrelated settings');
  await nav.getByRole('button',{name:'首页',exact:true}).click();
  await page.evaluate(()=>window.finishRepair());
  await nav.getByRole('button',{name:'系统工具',exact:true}).click();
  await page.getByText('夹具修复完成',{exact:true}).waitFor();
  if(await page.evaluate(()=>window.calls.filter(c=>c.cmd==='repair_windows').length)!==1)throw Error('Repair restarted across navigation');
  for(const width of [1280,760]){
    await page.setViewportSize({width,height:820});
    if(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth))throw Error('Horizontal overflow '+width);
  }
  await page.setViewportSize({width:1280,height:820});
  await page.screenshot({path:'output/playwright/system-tools.png',fullPage:true});
  console.log('PASS: 9 system flows: no beautification, suggested-app preference, lazy reads, isolated read failure, unset preference off, original restore, native tool route, independent repair/navigation, responsive layout');
}
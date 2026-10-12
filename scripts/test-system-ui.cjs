async (page) => {
  await page.addInitScript(() => {
    window.isTauri=true;window.calls=[];let cb=0;
    window.__TAURI_INTERNALS__={transformCallback:()=>++cb,unregisterCallback:()=>{},invoke:async(cmd,args)=>{
      window.calls.push({cmd,args});
      if(cmd==='get_optimization_presets')return [{id:'privacy-ads',label:'关闭广告与推荐',description:'推广',enabled:false,canRestore:false,error:null}];
      if(cmd==='repair_windows')return new Promise(resolve=>window.finishRepair=()=>resolve({success:true,message:'夹具修复完成',logs:'fixture only',restart:false}));
      if(['storage_drives','list_components','check_component_updates'].includes(cmd))return [];return null;
    }};
  });
  await page.goto('http://127.0.0.1:1420');await page.setViewportSize({width:1280,height:820});
  const nav=page.getByRole('navigation',{name:'主导航'});await nav.getByRole('button',{name:'系统工具',exact:true}).click();
  if(await page.getByRole('switch').count())throw Error('Optimization switches duplicated in system page');
  if(await page.getByRole('button',{name:/视觉效果|任务栏设置|隐私与推荐内容|电源与睡眠|后台录制/}).count())throw Error('Obsolete shortcut remains');
  if(await page.getByRole('button',{name:/默认应用/}).count()!==1)throw Error('Duplicate default-app management');
  await page.getByRole('button',{name:/设备与驱动/}).click();if(!await page.evaluate(()=>window.calls.some(c=>c.cmd==='open_health_tool'&&c.args.tool==='open_devices')))throw Error('Device diagnostic route wrong');
  await page.getByRole('button',{name:'检查并修复',exact:true}).click();await page.getByRole('button',{name:'修复中…',exact:true}).waitFor();
  await nav.getByRole('button',{name:'一键优化',exact:true}).click();await page.locator('.optimization-options > summary').click();
  if(await page.getByRole('switch',{name:'关闭广告与推荐',exact:true}).isDisabled())throw Error('System repair blocks unrelated setting');
  await page.evaluate(()=>window.finishRepair());await nav.getByRole('button',{name:'系统工具',exact:true}).click();await page.getByText('夹具修复完成',{exact:true}).waitFor();
  if(await page.evaluate(()=>window.calls.filter(c=>c.cmd==='repair_windows').length)!==1)throw Error('Repair restarted across navigation');
  for(const width of [1280,760]){await page.setViewportSize({width,height:820});if(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth))throw Error('Horizontal overflow');}
  console.log('PASS: settings consolidated, obsolete shortcuts removed, unique management entry, device route, independent repair, retained repair state, layout');
}

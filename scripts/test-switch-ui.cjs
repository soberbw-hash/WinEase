async (page) => {
  await page.addInitScript(() => {
    window.isTauri=true;window.calls=[];let cb=0, mode='balanced';
    let settings=[{id:'autoplay',label:'移动设备自动播放',description:'设备',enabled:null,canRestore:false,error:null},{id:'local-search',label:'本地搜索',description:'搜索',enabled:true,canRestore:false,error:null}];
    window.__TAURI_INTERNALS__={transformCallback:()=>++cb,unregisterCallback:()=>{},invoke:async(cmd,args)=>{
      window.calls.push({cmd,args});
      if(cmd==='get_windows_settings')return settings.map(s=>({...s}));
      if(cmd==='get_power_plan')return {mode,performanceAvailable:true};
      if(cmd==='set_power_plan'){if(window.failPower)throw '夹具电源切换失败';mode=args.mode;return null;}
      if(cmd==='set_windows_setting'){
        const finish=()=>{settings=settings.map(s=>s.id===args.id?{...s,enabled:args.enabled,canRestore:true}:s);return null;};
        if(window.holdSetting)return new Promise(resolve=>window.finishSetting=()=>resolve(finish()));
        return finish();
      }
      if(cmd==='restore_windows_setting'){settings=settings.map(s=>s.id===args.id?{...s,enabled:null,canRestore:false}:s);return null;}
      if(cmd==='start_optimization_check')return {id:'fixture',revision:1,phase:'ready',checkedAt:'',sections:[],outcomes:[],completedActions:0,totalActions:0};
      if(cmd==='optimization_status')return null;
      if(['list_components','storage_drives','check_component_updates','list_processes'].includes(cmd))return [];
      return null;
    }};
  });
  await page.goto('http://127.0.0.1:1420');await page.setViewportSize({width:1280,height:820});
  const area=page.getByRole('region',{name:'优化设置'}), nav=page.getByRole('navigation',{name:'主导航'});
  await area.getByRole('switch',{name:'高性能',exact:true}).waitFor();
  if(await area.getByRole('combobox').count())throw Error('Settings still use dropdowns');
  const auto=area.getByRole('switch',{name:'移动设备自动播放',exact:true});
  await area.getByText('系统默认',{exact:true}).waitFor();
  await auto.focus();await page.keyboard.press('Space');
  if(!await auto.isChecked())throw Error('Keyboard switch did not activate');
  await auto.uncheck();
  const row=page.locator('.setting-line').filter({has:page.getByRole('switch',{name:'移动设备自动播放',exact:true})});await row.getByRole('button',{name:'恢复原设置'}).click();
  await area.getByText('系统默认',{exact:true}).waitFor();
  const power=area.getByRole('switch',{name:'高性能',exact:true});
  await power.check();await area.getByText('高性能',{exact:true}).last().waitFor();
  await power.uncheck();await area.getByText('默认',{exact:true}).waitFor();
  if(!await page.evaluate(()=>window.calls.some(c=>c.cmd==='set_power_plan'&&c.args.mode==='balanced')&&window.calls.some(c=>c.cmd==='set_power_plan'&&c.args.mode==='performance')))throw Error('Wrong power mapping');
  await page.evaluate(()=>window.failPower=true);await power.click();
  await area.getByText('夹具电源切换失败',{exact:true}).waitFor();if(await power.isChecked())throw Error('Failed power switch changed state');
  await page.evaluate(()=>{window.failPower=false;window.holdSetting=true;});await auto.click();
  if(await power.isDisabled())throw Error('Setting blocks unrelated power switch');
  await power.check();await page.evaluate(()=>window.finishSetting());
  await nav.getByRole('button',{name:'系统工具',exact:true}).click();
  if(await page.getByRole('switch',{name:'高性能',exact:true}).isVisible())throw Error('Settings duplicated in system page');
  await nav.getByRole('button',{name:'一键优化',exact:true}).click();
  await area.getByRole('switch',{name:'高性能',exact:true}).waitFor();
  await page.getByRole('button',{name:'一键检查',exact:true}).click();
  await page.evaluate(()=>window.holdSetting=false);await auto.uncheck();
  await page.getByText('设置已更改，重新检查后可执行建议项。',{exact:true}).waitFor();
  for(const width of [1280,760]){await page.setViewportSize({width,height:820});if(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth))throw Error('Layout overflow');}
  await page.setViewportSize({width:1280,height:820});await page.screenshot({path:'output/playwright/optimization-switches.png',fullPage:true});
  console.log('PASS: settings consolidated, no dropdowns, keyboard/default/restore, power on/off, failed state, independent actions, retained navigation, stale advice and layout');
}

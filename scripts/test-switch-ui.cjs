async (page) => {
  await page.addInitScript(() => {
    window.isTauri=true;window.calls=[];let cb=0;
    let presets=[['privacy-ads','关闭广告与推荐'],['diagnostics','关闭可选诊断与反馈'],['notifications','关闭应用通知'],['game-mode','启用游戏模式'],['recording','关闭游戏录制'],['delivery-upload','关闭更新共享上传'],['storage-cleanup','启用安全自动清理'],['disk-schedule','暂停自动磁盘优化'],['power','高性能'],['local-search','搜索仅显示本地内容'],['autoplay','关闭移动设备自动播放']].map(([id,label])=>({id,label,description:'固定配置范围',enabled:false,canRestore:false,error:null}));
    window.__TAURI_INTERNALS__={transformCallback:()=>++cb,unregisterCallback:()=>{},invoke:async(cmd,args)=>{
      window.calls.push({cmd,args});
      if(cmd==='get_optimization_presets')return presets.map(s=>({...s}));
      if(cmd==='set_optimization_preset'){
        if(!args.confirmed)throw '缺少确认';if(window.failPower&&args.id==='power')throw '夹具切换失败';
        const finish=()=>{presets=presets.map(s=>s.id===args.id?{...s,enabled:args.enabled,canRestore:true}:s);return '已保存并校验';};
        if(window.holdSetting&&args.id==='privacy-ads')return new Promise(resolve=>window.finishSetting=()=>resolve(finish()));return finish();
      }
      if(cmd==='start_optimization_check')return {id:'fixture',revision:1,phase:'ready',checkedAt:'',sections:[{id:'preferences',title:'优化设置',status:'complete',summary:'固定配置',items:presets.filter(p=>!p.enabled).map(p=>({id:'preference-'+p.id,title:p.label,detail:p.description,status:'recommended',sizeBytes:0,fileCount:0,selectable:true,defaultSelected:true,actionLabel:'应用优化设置',target:'open_home',iconKind:'preferences',hasFiles:false}))}],outcomes:[],completedActions:0,totalActions:0};
      if(cmd==='start_optimization')return {id:'fixture',revision:2,phase:'complete',checkedAt:'',sections:[],outcomes:[],completedActions:args.itemIds.length,totalActions:args.itemIds.length};
      if(cmd==='optimization_status')return null;
      if(['list_components','storage_drives','check_component_updates','list_processes'].includes(cmd))return [];
      return null;
    }};
  });
  await page.goto('http://127.0.0.1:1420');await page.setViewportSize({width:1280,height:820});
  const area=page.getByRole('region',{name:'优化设置'}), fold=page.locator('.optimization-options > summary'), nav=page.getByRole('navigation',{name:'主导航'});
  if(await area.isVisible())throw Error('Optimization options expanded by default');
  if(await page.getByText('先检查，再确认优化',{exact:true}).count())throw Error('Removed copy still present');
  await page.screenshot({path:'output/playwright/optimization-collapsed.png',fullPage:true});
  await fold.click();await area.getByRole('switch',{name:'高性能',exact:true}).waitFor();
  if(await area.getByRole('combobox').count()||await area.getByRole('switch').count()!==11)throw Error('Preset switches missing');
  if(await area.getByRole('button',{name:/恢复/}).count())throw Error('Redundant restore buttons');
  const ads=area.getByRole('switch',{name:'关闭广告与推荐',exact:true}),power=area.getByRole('switch',{name:'高性能',exact:true});
  await ads.focus();await page.keyboard.press('Space');if(!await ads.isChecked())throw Error('Keyboard switch not applied');await ads.uncheck();
  await power.check();await power.uncheck();
  if(!await page.evaluate(()=>window.calls.some(c=>c.cmd==='set_optimization_preset'&&c.args.id==='power'&&c.args.enabled===false&&c.args.confirmed)))throw Error('Off does not restore preset');
  await page.evaluate(()=>window.failPower=true);await power.click();await area.getByText('夹具切换失败',{exact:true}).waitFor();if(await power.isChecked())throw Error('Failed state changed');
  await page.evaluate(()=>{window.failPower=false;window.holdSetting=true;});await ads.click();if(await power.isDisabled())throw Error('Unrelated switch blocked');await power.check();await page.evaluate(()=>window.finishSetting());
  await nav.getByRole('button',{name:'系统工具',exact:true}).click();if(await page.getByRole('button',{name:/隐私与推荐内容|诊断与反馈|电源与睡眠|后台录制|传递优化/}).count())throw Error('Obsolete settings shortcuts remain');
  await nav.getByRole('button',{name:'一键优化',exact:true}).click();await area.getByRole('switch',{name:'高性能',exact:true}).waitFor();
  await page.getByRole('button',{name:'一键检查',exact:true}).click();
  const choices=page.getByRole('checkbox',{name:/^选择 /});if(await choices.count()!==9)throw Error('Default optimization scope missing');for(const choice of await choices.all())if(!await choice.isChecked())throw Error('Optimization not recommended by default');
  await page.getByRole('region',{name:'检查概览'}).getByRole('button',{name:'一键优化',exact:true}).click();
  const dialog=page.getByRole('alertdialog');if(!(await dialog.textContent()).includes('暂停定期磁盘维护'))throw Error('Consent omitted effect');await dialog.getByRole('button',{name:'取消',exact:true}).click();
  if(await page.evaluate(()=>window.calls.some(c=>c.cmd==='start_optimization')))throw Error('Applied without consent');
  await page.evaluate(()=>window.holdSetting=false);await ads.uncheck();await page.getByText('设置已更改，重新检查后可执行建议项。',{exact:true}).waitFor();
  if(!await page.getByRole('region',{name:'检查概览'}).getByRole('button',{name:'一键优化',exact:true}).isDisabled())throw Error('Stale advice executes');
  for(const width of [1280,760]){await page.setViewportSize({width,height:820});if(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth))throw Error('Layout overflow');}
  await page.setViewportSize({width:1280,height:820});await page.screenshot({path:'output/playwright/optimization-presets.png',fullPage:true});
  console.log('PASS: collapsed default, clean copy, 11 bundled toggles, keyboard/restore, failure, independent task, no obsolete shortcuts, default recommendations, consent, stale plan, layout');
}

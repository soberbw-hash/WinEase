async (page) => {
  await page.addInitScript(() => {
    window.isTauri = true; window.calls = []; let callback = 0;
    const item = (id,title,status,more={}) => ({id,title,status,detail:'检测依据与处理建议',sizeBytes:0,fileCount:0,selectable:false,defaultSelected:false,actionLabel:null,target:null,iconTarget:null,iconKind:'system',hasFiles:false,...more});
    window.report = {id:'fixture-report',revision:0,phase:'checking',checkedAt:new Date().toISOString(),outcomes:[],completedActions:0,totalActions:0,sections:[['cleaning','深度清理'],['files','大文件与空间'],['health','电脑体检'],['network','网络检测'],['startup','开机管理']].map(([id,title])=>({id,title,status:'checking',summary:'检查中…',items:[]}))};
    window.partial = () => {
      window.report.revision++;
      window.report.sections[0] = {...window.report.sections[0],status:'complete',summary:'发现 600 MB 可清理缓存',items:[
        item('cache','Chrome · 网页缓存','recommended',{sizeBytes:600*1024*1024,fileCount:130,selectable:true,defaultSelected:true,actionLabel:'清理缓存',hasFiles:true,iconTarget:'C:\\Chrome.exe',iconKind:'application'}),
        item('shader','NVIDIA · 着色器缓存','optional',{sizeBytes:100*1024*1024,selectable:true,actionLabel:'清理缓存',hasFiles:true})
      ]};
      window.report.sections[1] = {...window.report.sections[1],status:'complete',summary:'大文件检查完成',items:[item('duplicate-cache','cached-large.bin','optional',{sizeBytes:200*1024**2,selectable:true,actionLabel:'移到回收站'})]};
    };
    window.ready = () => {
      window.report.revision++; window.report.phase='ready';
      window.report.sections.slice(1).forEach(section=>{section.status='complete';section.summary='检查完成';});
      window.report.sections[1].items=[
        item('large','archive.zip','optional',{sizeBytes:3*1024**3,selectable:true,actionLabel:'移到回收站',iconTarget:'C:\\Downloads\\archive.zip',iconKind:'file',target:'C:\\Downloads',detail:'超过 30 天未修改的压缩包，确认用途后处理'}),
        item('protected','application.dll','optional',{sizeBytes:1024**3,detail:'应用文件仅供查看，不能直接删除',iconTarget:'C:\\Program Files\\application.dll',iconKind:'file'})
      ];
      window.report.sections[2].items=[item('memory','内存压力','attention',{detail:'占用 92%，建议关闭不用的应用',target:'open_processes'}),item('security','病毒防护','healthy',{detail:'实时防护已开启'})];
      window.report.sections[3].items=[item('network','网络地址修复','optional',{selectable:true,actionLabel:'备份并修复',detail:'可能短暂断网，需要确认'})];
      window.report.sections[4].items=[item('steam','Steam','recommended',{selectable:true,defaultSelected:true,actionLabel:'关闭开机启动',detail:'可关闭开机启动，仍可手动打开',iconTarget:'"C:\\Steam.exe" -silent',iconKind:'command'}),item('clash','Clash','info',{detail:'保留正常代理',iconTarget:'C:\\Clash.exe',iconKind:'application'})];
    };
    window.complete = () => {window.report.revision++;window.report.phase='complete';window.report.completedActions=window.report.totalActions;window.report.outcomes=[{itemIds:['cache'],title:'深度清理',status:'success',message:'已清理 130 个文件，释放 600 MB。'},{itemIds:['steam'],title:'开机优化',status:'failed',message:'启动项已变化，请重新检查'}];};
    window.__TAURI_INTERNALS__={transformCallback:()=>++callback,unregisterCallback:()=>{},invoke:async(cmd,args)=>{
      window.calls.push({cmd,args});
      if(cmd==='start_optimization_check') return structuredClone(window.report);
      if(cmd==='optimization_status')return args.revision===window.report.revision?null:structuredClone(window.report);
      if(cmd==='start_optimization'){window.report.phase='optimizing';window.report.revision++;window.report.totalActions=args.itemIds.length;return structuredClone(window.report);}
      if(cmd==='optimization_files')return {total:130,files:Array.from({length:args.offset?30:100},(_,i)=>({id:i+args.offset,name:`cache-${i+args.offset}.bin`,path:`C:\\Cache\\cache-${i+args.offset}.bin`,sizeBytes:4096}))};
      if(cmd==='list_components'||cmd==='storage_drives'||cmd==='check_component_updates')return [];
      if(cmd==='get_application_icon'||cmd==='get_file_icon'||cmd==='plugin:updater|check')return null;
      return null;
    }};
  });
  await page.goto('http://127.0.0.1:1420'); await page.setViewportSize({width:1280,height:820});
  await page.screenshot({path:'output/playwright/optimization-home.png'});
  await page.getByRole('button',{name:'一键检查',exact:true}).click();
  await page.getByRole('heading',{name:'正在检查',exact:true}).waitFor();
  if(!await page.getByRole('button',{name:'一键优化',exact:true}).isDisabled())throw Error('Allowed apply while checking');
  await page.evaluate(()=>window.partial());
  await page.getByRole('checkbox',{name:'选择 Chrome · 网页缓存',exact:true}).waitFor();
  if(!await page.getByRole('checkbox',{name:'选择 Chrome · 网页缓存',exact:true}).isChecked())throw Error('Recommended cache not selected');
  await page.getByRole('checkbox',{name:'选择 Chrome · 网页缓存',exact:true}).uncheck();
  await page.getByRole('checkbox',{name:'选择 cached-large.bin',exact:true}).check();
  await page.evaluate(()=>window.ready());
  await page.getByRole('heading',{name:'检查结果',exact:true}).waitFor();
  if(await page.getByRole('checkbox',{name:'选择 Chrome · 网页缓存',exact:true}).isChecked())throw Error('Later results reset manual choice');
  if(!await page.locator('.optimization-summary').textContent().then(t=>t.includes('已选 1 项')))throw Error('Removed duplicate left a stale selected action');
  if(await page.getByRole('checkbox',{name:'选择 archive.zip',exact:true}).isChecked())throw Error('Personal file default selected');
  if(!await page.getByRole('checkbox',{name:'选择 Steam',exact:true}).isChecked())throw Error('Recommended startup not selected');
  if(await page.getByRole('checkbox',{name:'选择 application.dll',exact:true}).count())throw Error('Protected file selectable');
  await page.getByRole('navigation',{name:'主导航'}).getByRole('button',{name:'组件',exact:true}).click();
  await page.getByRole('navigation',{name:'主导航'}).getByRole('button',{name:'首页',exact:true}).click();
  await page.getByRole('heading',{name:'检查结果',exact:true}).waitFor();
  if(await page.evaluate(()=>window.calls.filter(c=>c.cmd==='start_optimization_check').length)!==1)throw Error('Navigation restarted scan');
  const cache=page.locator('.optimization-item').filter({has:page.getByText('Chrome · 网页缓存',{exact:true})});
  await cache.getByRole('button',{name:'查看文件',exact:true}).click();await cache.getByText('cache-0.bin',{exact:true}).waitFor();
  await cache.getByRole('button',{name:/更多文件/}).click();await cache.getByText('cache-129.bin',{exact:true}).waitFor();
  await cache.getByRole('button',{name:'收起文件',exact:true}).click();
  await page.getByRole('button',{name:'使用建议项',exact:true}).click();
  await page.getByRole('button',{name:'一键优化',exact:true}).click();
  await page.getByRole('alertdialog').waitFor();
  if(await page.evaluate(()=>window.calls.some(c=>c.cmd==='start_optimization')))throw Error('Applied before confirmation');
  await page.getByRole('button',{name:'取消',exact:true}).click();
  await page.getByRole('checkbox',{name:'选择 archive.zip',exact:true}).check();
  await page.getByRole('button',{name:'一键优化',exact:true}).click();
  if(!await page.getByRole('alertdialog').textContent().then(t=>t.includes('移到回收站')))throw Error('Personal-file effect absent');
  await page.getByRole('button',{name:'取消',exact:true}).click();
  await page.getByRole('button',{name:'使用建议项',exact:true}).click();
  await page.screenshot({path:'output/playwright/optimization-results.png'});
  await page.getByRole('button',{name:'一键优化',exact:true}).click();
  await page.getByRole('button',{name:'确认并优化',exact:true}).click();
  await page.getByRole('heading',{name:'正在优化',exact:true}).waitFor();
  const call=await page.evaluate(()=>window.calls.filter(c=>c.cmd==='start_optimization'));
  if(call.length!==1||call[0].args.itemIds.sort().join(',')!=='cache,steam'||call[0].args.confirmed!==true)throw Error('Wrong approved IDs');
  await page.evaluate(()=>window.complete());await page.getByRole('heading',{name:'优化结果',exact:true}).waitFor();
  await page.getByRole('region',{name:'执行结果'}).getByText('启动项已变化，请重新检查',{exact:true}).waitFor();
  for(const [width,height] of [[1280,820],[760,520]]){
    await page.setViewportSize({width,height});
    if(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth||document.querySelector('.page-frame').scrollWidth>document.querySelector('.page-frame').clientWidth))throw Error(`Horizontal overflow at ${width}`);
  }
  await page.screenshot({path:'output/playwright/optimization-small.png'});
  return {passed:['parallel progress','recommendations and manual choices','personal/protected files preserved by default','cross-page retention','cache pagination','no optimization before consent or after cancel','confirmed ID-only apply','partial failure shown','1280 and 760 layouts']};
}

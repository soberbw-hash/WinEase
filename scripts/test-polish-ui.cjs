async (page) => {
  await page.addInitScript(() => {
    window.isTauri=true;window.calls=[];let cb=0;
    const files=Array.from({length:64},(_,i)=>({id:i,name:`file-${i}.${['safetensors','mp4','zip','png','exe','mp3','txt'][i%7]}`,path:`C:\\Users\\user\\Documents\\file-${i}.${['safetensors','mp4','zip','png','exe','mp3','txt'][i%7]}`,sizeBytes:(i+1)*1024**2,fileCount:1,group:null,canRecycle:false,modifiedAt:0}));
    const total=files.reduce((n,f)=>n+f.sizeBytes,0);
    window.fixture={scanId:'fixture',files,usage:['C:\\Users','C:\\Users\\user','C:\\Users\\user\\Documents'].map(path=>({path,name:path.split('\\').pop(),sizeBytes:total,fileCount:64})),totalBytes:total,skipped:0,limited:false,breakdown:[]};
    const components=[['piik-share','Piik 屏幕共享'],['rustdesk-remote','RustDesk 远程协助'],['localsend','LocalSend 文件传输']].map(([id,name])=>({id,name,kind:'github',installed:true,statusLabel:'可用',supportsRepair:true,supportsUpdate:true,supportsUninstall:true,summary:'共享与协作',category:'共享与协作',logDir:null}));
    window.__TAURI_INTERNALS__={transformCallback:()=>++cb,unregisterCallback:()=>{},invoke:async(cmd,args)=>{
      window.calls.push({cmd,args});
      if(cmd==='storage_drives')return [{path:'C:\\',totalBytes:1024**4,freeBytes:1024**3}];
      if(cmd==='scan_personal_files')return window.fixture;
      if(cmd==='list_components')return components;
      if(cmd==='check_component_updates')return [];
      if(cmd==='get_optimization_presets')return Array.from({length:16},(_,i)=>({id:`preset-${i}`,label:`优化 ${i}`,description:'固定配置范围',enabled:false,canRestore:false,error:null}));
      if(cmd==='start_optimization_check')return {id:'fixture-report',revision:1,phase:'ready',checkedAt:'',outcomes:[],completedActions:0,totalActions:0,sections:['cleaning','files','health','network','startup','resources','maintenance','preferences','apps'].map(id=>({id,title:id,status:'complete',summary:'检查完成',items:Array.from({length:16},(_,i)=>({id:`${id}-${i}`,title:`${id} 项目 ${i}`,detail:'说明',sizeBytes:0,fileCount:0,selectable:id==='preferences',defaultSelected:id==='preferences',status:id==='preferences'?'recommended':'attention',actionLabel:id==='preferences'?'应用优化设置':null,hasFiles:false}))}))};
      return null;
    }};
  });
  await page.goto('http://127.0.0.1:1420');await page.setViewportSize({width:1440,height:900});
  const nav=page.getByRole('navigation',{name:'主导航'});
  await page.getByRole('button',{name:'优化设置',exact:true}).click();
  await page.getByRole('switch',{name:'优化 0',exact:true}).waitFor();
  const columns=await page.locator('.optimization-setting').evaluateAll(rows=>rows.slice(0,2).map(r=>r.getBoundingClientRect().top));
  if(Math.abs(columns[0]-columns[1])>1)throw Error('Settings not in two columns');
  await page.waitForFunction(()=>{const r=document.querySelector('#optimization-settings').getBoundingClientRect();return r.top>=64&&r.top<180;});
  await page.evaluate(()=>new Promise(resolve=>{let last=-1,stable=0;function settle(){const top=document.querySelector('#optimization-settings').getBoundingClientRect().top;stable=Math.abs(top-last)<0.1?stable+1:0;last=top;if(stable>=6)resolve();else requestAnimationFrame(settle);}settle();}));
  await page.screenshot({path:'output/playwright/polish-settings.png'});
  await page.getByRole('button',{name:'一键检查',exact:true}).click();
  await page.getByRole('heading',{name:'检查结果',exact:true}).waitFor();
  if(await page.locator('.optimization-section[open]').count())throw Error('Long results expanded by default');
  if(await page.locator('.optimization-summary').evaluate(el=>getComputedStyle(el).position)==='sticky')throw Error('Summary still overlays results');
  if(await page.locator('.optimization-home').evaluate(el=>el.scrollHeight)>1600)throw Error('Collapsed results too tall');
  await page.locator('a[href="#check-preferences"]').click();
  if(!await page.locator('#check-preferences').evaluate(el=>el.open))throw Error('Overview does not open category');
  const checkboxes=page.locator('#check-preferences input[type=checkbox]');
  if(await checkboxes.count()!==10)throw Error('Initial results not bounded');
  for(const box of await checkboxes.all())if(!await box.isChecked())throw Error('Preference not selected by default');
  await page.locator('#check-preferences').getByRole('button',{name:/显示更多/}).click();
  if(await checkboxes.count()!==16)throw Error('Complete result inaccessible');
  await page.screenshot({path:'output/playwright/polish-results.png'});
  await nav.getByRole('button',{name:'空间管理',exact:true}).click();
  await page.getByRole('button',{name:'开始扫描',exact:true}).click();
  await page.locator('.disk-map__tile').first().waitFor();
  const tiles=await page.locator('.disk-map__tile').evaluateAll(nodes=>nodes.map(n=>({color:getComputedStyle(n).backgroundColor,x:parseFloat(n.style.left),y:parseFloat(n.style.top),width:parseFloat(n.style.width),height:parseFloat(n.style.height)})));
  if(tiles.length!==64||new Set(tiles.map(t=>t.color)).size<7)throw Error('Single Users chain not expanded into colored files');
  if(Math.abs(tiles.reduce((sum,t)=>sum+t.width*t.height,0)-10000)>0.03)throw Error('Treemap does not cover canvas');
  for(let i=0;i<tiles.length;i++)for(let j=i+1;j<tiles.length;j++){const a=tiles[i],b=tiles[j];if(Math.min(a.x+a.width,b.x+b.width)-Math.max(a.x,b.x)>0.001&&Math.min(a.y+a.height,b.y+b.height)-Math.max(a.y,b.y)>0.001)throw Error('Tiles overlap');}
  if(Math.abs(tiles[0].width*tiles[0].height/10000-64/2080)>0.0001)throw Error('Area not proportional to file size');
  await page.screenshot({path:'output/playwright/polish-diskmap.png',fullPage:true});
  await page.locator('.disk-map__tile').first().click();
  if(!await page.evaluate(()=>window.calls.some(c=>c.cmd==='open_target'&&c.args.target==='C:\\Users\\user\\Documents')))throw Error('Tile does not open containing folder');
  await nav.getByRole('button',{name:'设置',exact:true}).click();
  await page.getByRole('navigation',{name:'设置分类'}).getByRole('button',{name:'组件',exact:true}).click();
  if(await page.getByRole('heading',{name:'第三方许可证',exact:true}).count())throw Error('Large license panel remains');
  if(await page.locator('.installed-component-card').getByRole('button',{name:'更新',exact:true}).count()!==3)throw Error('Installed update controls inconsistent');
  if(await page.locator('.settings-component-name img').count()!==3)throw Error('Official component icons missing');
  await page.getByRole('navigation',{name:'设置分类'}).getByRole('button',{name:'关于',exact:true}).click();
  await page.getByRole('button',{name:'开源声明',exact:true}).click();await page.getByRole('dialog',{name:'开源声明'}).waitFor();await page.keyboard.press('Escape');
  for(const width of [1440,760]){await page.setViewportSize({width,height:900});if(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth))throw Error('Horizontal overflow');}
  return {passed:['two columns and automatic reveal scroll','collapsed compact result categories','default preference checks and pagination','overview opens category','64 proportional non-overlapping colorful tiles from one Users chain','tile opens local directory','consistent installed update controls','official icons','compact notices disclosure','wide/narrow layout']};
}

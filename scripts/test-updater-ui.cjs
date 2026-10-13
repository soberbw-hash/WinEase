async (page)=>{
  await page.addInitScript(()=>{
    window.isTauri=true;window.calls=[];window.updateMode='current';let callback=0;
    window.__TAURI_INTERNALS__={transformCallback:()=>++callback,unregisterCallback:()=>{},invoke:async(cmd,args)=>{
      window.calls.push({cmd,args});
      if(['list_components','check_component_updates','storage_drives','get_optimization_presets'].includes(cmd))return [];
      if(cmd==='plugin:updater|check'){
        if(window.updateMode==='error')throw 'fixture network failure';
        if(window.updateMode==='current')return null;
        return {rid:1,version:'1.0.1',currentVersion:'1.0.0',rawJson:{platforms:{'windows-x86_64':{url:window.updateMode==='bad'?'https://example.invalid/program.exe':'https://github.com/soberbw-hash/WinEase/releases/download/v1.0.1/WinEase_1.0.1_x64-setup.exe'}}}};
      }
      if(cmd==='plugin:updater|download')return new Promise(resolve=>window.finishDownload=()=>resolve(2));
      return null;
    }};
  });
  await page.goto('http://127.0.0.1:1420');
  const manual=()=>page.evaluate(()=>window.dispatchEvent(new Event('winease-check-update')));
  await manual();await page.getByText('当前已是最新版本。',{exact:true}).waitFor();
  await page.evaluate(()=>window.updateMode='error');await manual();await page.getByText('更新检查失败：fixture network failure',{exact:true}).waitFor();
  await page.evaluate(()=>window.updateMode='bad');await manual();await page.getByText('更新检查失败：Error: 更新下载地址不属于 WinEase 官方发布。',{exact:true}).waitFor();
  if(await page.evaluate(()=>window.calls.some(c=>c.cmd==='plugin:updater|download')))throw Error('Foreign package downloaded');
  await page.evaluate(()=>window.updateMode='new');await manual();await page.getByText('后台下载更新…',{exact:true}).waitFor();
  if(await page.evaluate(()=>window.calls.some(c=>c.cmd==='plugin:updater|install')))throw Error('Installed before download/consent');
  await page.evaluate(()=>window.finishDownload());const dialog=page.getByRole('alertdialog');await dialog.waitFor();
  await dialog.getByRole('button',{name:'取消',exact:true}).click();
  if(await page.evaluate(()=>window.calls.some(c=>c.cmd==='plugin:updater|install')))throw Error('Installed after cancel');
  await page.getByRole('button',{name:'新版 1.0.1 已下载 · 安装并重启',exact:true}).click();
  await dialog.getByRole('button',{name:'安装并重启',exact:true}).click();
  const installed=await page.evaluate(()=>window.calls.filter(c=>c.cmd==='plugin:updater|install'));
  if(installed.length!==1||installed[0].args.restartAfterInstall!==true)throw Error('Confirmed restart installation incorrect');
  return {passed:['current-version result','network error not misreported as current','foreign download rejected','background download','no install before confirmation or after cancel','confirmed install with restart']};
}

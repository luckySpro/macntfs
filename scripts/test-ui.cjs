// UI contract tests use an isolated browser and mocked IPC; no physical disk is touched.
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const fs=require('fs');
(async()=>{
 const browser=await chromium.launch({headless:true,executablePath:process.env.CHROME_EXECUTABLE||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'});const page=await browser.newPage({viewport:{width:1440,height:1024}});const errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.addInitScript(()=>{
 const callbacks=new Map();let serial=0;let settings=JSON.parse(localStorage.getItem('qa-settings')||'{"backend":"Auto","dark":false,"auto_mount":true,"theme":"Stone"}');
 window.__QA_OPS=[];
 window.__TAURI_INTERNALS__={transformCallback(cb){const id=++serial;callbacks.set(id,cb);return id},unregisterCallback(id){callbacks.delete(id)},invoke:async(cmd,args)=>{
 if(cmd==='snapshot')return {version:'0.3.8',volumes:JSON.parse(localStorage.getItem('qa-volumes')||'null')||[{uuid:'test-backup',name:'BackUp',id:'disk4s3',size:511700000000,mount:'/Volumes/NTFS-disk4s3',writable:true}],settings,environment:{runtime:true,fuse:true,service:true,os:'26.0',runtime_version:'0.3.8',fuse_version:'5.1.3'},monitor:{busy:false,last_event:''}};
 if(cmd==='save_settings'){settings=args.settings;localStorage.setItem('qa-settings',JSON.stringify(settings));return null}
 if(cmd==='operate'){window.__QA_OPS.push(args);return '操作完成'}
 if(cmd==='check_updates')return null;
 if(cmd==='plugin:event|listen')return ++serial;
 if(cmd==='plugin:event|unlisten')return null;
 throw new Error('Unexpected IPC '+cmd);
 }};
 });
 await page.goto(process.env.TEST_UI_URL||'http://127.0.0.1:1420');await page.getByRole('heading',{name:'BackUp',exact:true}).waitFor();fs.mkdirSync('/tmp/macntfs-qa',{recursive:true});
 for(const theme of ['Stone','Office','Graphite']){
 await page.getByLabel('界面风格').selectOption(theme);await page.waitForTimeout(150);if(await page.locator('.dismiss').count())await page.locator('.dismiss').click();await page.screenshot({path:`/tmp/macntfs-qa/${theme}.png`,fullPage:true});
 await page.reload();await page.getByRole('heading',{name:'BackUp',exact:true}).waitFor();if(await page.getByLabel('界面风格').inputValue()!==theme)throw Error('theme not persisted');
 await page.getByRole('button',{name:'打开 Finder',exact:true}).click();await page.getByRole('button',{name:'安全推出',exact:true}).click();
 const ops=await page.evaluate(()=>window.__QA_OPS);if(ops.at(-2).action!=='open'||ops.at(-1).action!=='eject')throw Error('wrong disk actions');
 await page.setViewportSize({width:1060,height:760});await page.screenshot({path:`/tmp/macntfs-qa/${theme}-native.png`,fullPage:true});
 const overflow=await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth);if(overflow)throw Error(theme+' horizontal overflow');
 await page.setViewportSize({width:760,height:600});if(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth))throw Error(theme+' minimum width overflow');
 await page.setViewportSize({width:1440,height:1024});
 }
 await page.getByRole('button',{name:'设置与更新',exact:true}).click();await page.getByRole('heading',{name:'关于 macntfs',exact:true}).waitFor();await page.screenshot({path:'/tmp/macntfs-qa/settings.png',fullPage:true});
 await page.evaluate(()=>localStorage.setItem('qa-volumes','[]'));await page.reload();await page.getByRole('heading',{name:'等待连接磁盘'}).waitFor();
 await page.evaluate(()=>localStorage.setItem('qa-volumes',JSON.stringify([{uuid:'a',name:'ReadOnly',id:'disk5s1',size:1000000000,mount:'/Volumes/ReadOnly',writable:false},{uuid:'b',name:'Disconnected',id:'disk6s1',size:2000000000,mount:'',writable:false}])));await page.reload();await page.getByRole('heading',{name:'ReadOnly',exact:true}).waitFor();await page.getByRole('button',{name:'开启读写',exact:true}).click();if((await page.evaluate(()=>window.__QA_OPS)).at(-1).volume.uuid!=='a')throw Error('wrong selected volume');
 await page.locator('.device-nav button').filter({hasText:'Disconnected'}).click();await page.getByRole('heading',{name:'Disconnected',exact:true}).waitFor();await page.getByRole('button',{name:'开启读写',exact:true}).click();if((await page.evaluate(()=>window.__QA_OPS)).at(-1).volume.uuid!=='b')throw Error('wrong switched volume');
 if(errors.length)throw Error(errors.join('\n'));console.log('PASS: three themes persist; Finder/eject IPC; settings; native/minimum width; no page errors');await browser.close();
})().catch(e=>{console.error(e);process.exit(1)});

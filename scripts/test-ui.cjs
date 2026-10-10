// UI contract tests use an isolated browser and mocked IPC; no physical disk is touched.
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const fs=require('fs');
(async()=>{
 const browser=await chromium.launch({headless:true,executablePath:process.env.CHROME_EXECUTABLE||'/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'});const page=await browser.newPage({viewport:{width:1440,height:1024}});const errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.addInitScript(()=>{
 const callbacks=new Map();let serial=0;let settings=JSON.parse(localStorage.getItem('qa-settings')||'{"backend":"Auto","dark":false,"auto_mount":true,"theme":"Stone","language":"zh-Hans"}');
 window.__QA_OPS=[];window.__QA_WINDOW=[];
 window.__TAURI_INTERNALS__={transformCallback(cb){const id=++serial;callbacks.set(id,cb);return id},unregisterCallback(id){callbacks.delete(id)},invoke:async(cmd,args)=>{
 if(cmd==='snapshot')return {version:'0.3.12',required_update:JSON.parse(localStorage.getItem('qa-required')||'null'),update_blockers:Number(localStorage.getItem('qa-blockers')||0),volumes:JSON.parse(localStorage.getItem('qa-volumes')||'null')||[{uuid:'test-backup',name:'BackUp',id:'disk4s3',size:511700000000,mount:'/Volumes/NTFS-disk4s3',writable:true}],settings,environment:{runtime:true,fuse:true,service:true,microvm:true,os:'26.0',runtime_version:'0.3.8',fuse_version:'5.1.3'},monitor:{busy:!!localStorage.getItem('qa-busy'),last_event:localStorage.getItem('qa-event')||''}};
 if(cmd==='diagnose')return {schema:1,version:'test',backend:settings.backend,checks:[{code:'helper',title:'后台助手',status:'pass',detail:'已验证助手连接与版本',action:''},{code:'permission',title:'磁盘访问权限',status:'unknown',detail:'助手连接不能证明磁盘访问权限；连接磁盘并尝试挂载后确认',action:'permissions'}],volumes:[{index:1,mounted:true,writable:true}]};
 if(cmd==='set_auto_mount'){settings.auto_mount=args.enabled;localStorage.setItem('qa-settings',JSON.stringify(settings));return null}
 if(cmd==='save_settings'){settings=args.settings;localStorage.setItem('qa-settings',JSON.stringify(settings));return null}
 if(cmd==='operate'){if(localStorage.getItem('qa-error'))throw '模拟操作失败';window.__QA_OPS.push(args);return '操作完成'}
 if(cmd==='check_updates'){if(localStorage.getItem('qa-check-error'))throw 'offline';return JSON.parse(localStorage.getItem('qa-required')||'null')}
 if(cmd==='install_update'){if(localStorage.getItem('qa-check-error'))throw 'offline';window.__QA_WINDOW.push('install');throw 'simulated download failure'}
 if(cmd==='panel_action'){window.__QA_WINDOW.push(args.action);return null}
 if(cmd==='plugin:event|listen')return ++serial;
 if(cmd==='plugin:event|unlisten')return null;
 throw new Error('Unexpected IPC '+cmd);
 }};
 });
 await page.goto(process.env.TEST_UI_URL||'http://127.0.0.1:1420');await page.getByRole('heading',{name:'BackUp',exact:true}).waitFor();fs.mkdirSync('/tmp/macntfs-qa',{recursive:true});
 if(await page.locator('.header-actions select').count())throw Error('native theme selector remains');
 for(const theme of ['Stone','Office','Graphite']){
 await page.getByRole('button',{name:'设置与更新',exact:true}).click();await page.locator('.theme-options button').filter({hasText:({Stone:'暖灰原生',Office:'简洁办公',Graphite:'深色工作台'})[theme]}).click();await page.getByRole('button',{name:'我的磁盘',exact:true}).click();await page.waitForTimeout(150);if(await page.locator('.dismiss').count())await page.locator('.dismiss').click();await page.screenshot({path:`/tmp/macntfs-qa/${theme}.png`,fullPage:true});
 await page.reload();await page.getByRole('heading',{name:'BackUp',exact:true}).waitFor();if(await page.locator('.shell').getAttribute('data-theme')!==theme)throw Error('theme not persisted');
 await page.getByRole('button',{name:'打开 Finder',exact:true}).click();await page.getByRole('button',{name:'安全推出',exact:true}).click();
 const ops=await page.evaluate(()=>window.__QA_OPS);if(ops.at(-2).action!=='open'||ops.at(-1).action!=='eject')throw Error('wrong disk actions');
 await page.setViewportSize({width:1060,height:760});await page.screenshot({path:`/tmp/macntfs-qa/${theme}-native.png`,fullPage:true});
 const overflow=await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth);if(overflow)throw Error(theme+' horizontal overflow');
 await page.setViewportSize({width:760,height:600});if(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth))throw Error(theme+' minimum width overflow');
 await page.setViewportSize({width:1440,height:1024});
 }
 await page.getByRole('button',{name:'设置与更新',exact:true}).click();await page.getByRole('heading',{name:'关于 macntfs',exact:true}).waitFor();
 await page.getByRole('button',{name:'开始检查',exact:true}).click();await page.locator('.diagnostic-check').last().waitFor();if(await page.locator('.diagnostic-check[data-status=unknown]').count()!==1)throw Error('unconfirmed disk permission marked green');await page.getByRole('button',{name:/微虚拟机（实验性/}).click();if(await page.locator('.backend-note').count()!==1)throw Error('experimental warning missing');await page.getByRole('button',{name:'稳定模式（推荐）',exact:true}).click();await page.screenshot({path:'/tmp/macntfs-qa/settings.png',fullPage:true});
 await page.evaluate(()=>localStorage.setItem('qa-volumes','[]'));await page.reload();await page.getByRole('heading',{name:'等待连接磁盘'}).waitFor();
 await page.evaluate(()=>localStorage.setItem('qa-volumes',JSON.stringify([{uuid:'a',name:'ReadOnly',id:'disk5s1',size:1000000000,mount:'/Volumes/ReadOnly',writable:false},{uuid:'b',name:'Disconnected',id:'disk6s1',size:2000000000,mount:'',writable:false}])));await page.reload();await page.getByRole('heading',{name:'ReadOnly',exact:true}).waitFor();await page.getByRole('button',{name:'开启读写',exact:true}).click();if((await page.evaluate(()=>window.__QA_OPS)).at(-1).volume.uuid!=='a')throw Error('wrong selected volume');
 await page.locator('.device-nav button').filter({hasText:'Disconnected'}).click();await page.getByRole('heading',{name:'Disconnected',exact:true}).waitFor();await page.getByRole('button',{name:'开启读写',exact:true}).click();if((await page.evaluate(()=>window.__QA_OPS)).at(-1).volume.uuid!=='b')throw Error('wrong switched volume');
 await page.setViewportSize({width:420,height:540});await page.evaluate(()=>localStorage.removeItem('qa-volumes'));await page.goto((process.env.TEST_UI_URL||'http://127.0.0.1:1420')+'/?panel');await page.locator('.panel-volume').waitFor();
 if(!(await page.getByRole('button',{name:'开启读写',exact:true}).isDisabled()))throw Error('writable volume allows mount');
 await page.getByRole('button',{name:'打开 Finder',exact:true}).click();await page.getByRole('button',{name:'安全推出',exact:true}).click();const panelOps=await page.evaluate(()=>window.__QA_OPS);if(panelOps.at(-2).action!=='open'||panelOps.at(-1).action!=='eject')throw Error('panel operations incorrect');
 await page.getByRole('switch',{name:'插入后自动开启读写'}).click();if(await page.getByRole('switch').getAttribute('aria-checked')!=='false')throw Error('panel toggle not saved');
 await page.getByRole('button',{name:'打开主窗口',exact:true}).click();await page.getByRole('button',{name:'设置与更新',exact:true}).click();await page.keyboard.press('Escape');const windows=await page.evaluate(()=>window.__QA_WINDOW);if(windows.join(',')!=='show,settings,hide')throw Error('panel navigation incorrect');
 await page.screenshot({path:'/tmp/macntfs-qa/panel.png'});
 for(const theme of ['Stone','Office','Graphite']){await page.evaluate(theme=>{const s=JSON.parse(localStorage.getItem('qa-settings'));s.theme=theme;localStorage.setItem('qa-settings',JSON.stringify(s))},theme);await page.reload();await page.locator('.panel-volume').waitFor();if(!(await page.evaluate(()=>[document.documentElement,document.body,document.querySelector('#app')].every(el=>getComputedStyle(el).backgroundColor==='rgba(0, 0, 0, 0)'))))throw Error('opaque popup document '+theme);await page.screenshot({path:`/tmp/macntfs-qa/panel-${theme}.png`,omitBackground:true});}
 await page.evaluate(()=>localStorage.setItem('qa-volumes',JSON.stringify(Array.from({length:6},(_,i)=>({uuid:'panel-'+i,name:'Disk '+i,id:'disk'+i+'s1',mount:'/Volumes/Disk'+i,writable:i%2===0,size:1000000000})))));await page.reload();await page.locator('.panel-volume').last().waitFor();
 if(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth||document.documentElement.scrollHeight>innerHeight))throw Error('panel viewport overflow');if(!(await page.evaluate(()=>{const el=document.querySelector('.panel-devices');return el.scrollHeight>el.clientHeight})))throw Error('multi-volume panel not scrollable');
 await page.getByRole('button',{name:'安全推出',exact:true}).last().click();if((await page.evaluate(()=>window.__QA_OPS)).at(-1).volume.uuid!=='panel-5')throw Error('panel targets wrong volume');
 await page.evaluate(()=>localStorage.setItem('qa-busy','1'));await page.reload();await page.locator('.panel-volume').last().waitFor();if(!(await page.getByRole('button',{name:'安全推出',exact:true}).last().isDisabled()))throw Error('panel allows concurrent eject');
 await page.evaluate(()=>{localStorage.removeItem('qa-busy');localStorage.setItem('qa-error','1')});await page.reload();await page.locator('.panel-volume').last().waitFor();await page.getByRole('button',{name:'安全推出',exact:true}).last().click();await page.getByText('模拟操作失败',{exact:true}).waitFor();if(await page.getByRole('button',{name:'安全推出',exact:true}).last().isDisabled())throw Error('failed panel operation stays locked');
 await page.evaluate(()=>{localStorage.removeItem('qa-error');localStorage.removeItem('qa-volumes')});
 await page.setViewportSize({width:1060,height:760});await page.goto((process.env.TEST_UI_URL||'http://127.0.0.1:1420'));
 for(const [id,label,heading,open,guide] of [['en','English','Settings & updates','Open Finder','Getting started'],['zh-Hant','繁體中文','設置與更新','打開 Finder','首次使用嚮導'],['ja','日本語','設定と更新','Finder で開く','初回設定ガイド'],['zh-Hans','简体中文','设置与更新','打开 Finder','首次使用向导']]){
 await page.locator('aside button').nth(2).click();await page.locator('.language-options button').filter({hasText:label}).click();await page.getByRole('heading',{name:heading,exact:true}).waitFor();
 if(await page.locator('html').getAttribute('lang')!==id)throw Error('wrong document language');
 if(id==='en'){await page.evaluate(()=>localStorage.setItem('qa-event','已在 Finder 中打开'));await page.getByRole('button',{name:'Refresh',exact:true}).click();await page.getByText(/Last action:.*Opened in Finder/).waitFor();}
 await page.screenshot({path:`/tmp/macntfs-qa/language-${id}.png`,fullPage:true});
 await page.reload();await page.getByRole('button',{name:open,exact:true}).waitFor();if(await page.locator('html').getAttribute('lang')!==id)throw Error('language lost after reload');
 await page.setViewportSize({width:420,height:540});await page.goto((process.env.TEST_UI_URL||'http://127.0.0.1:1420')+'/?panel');await page.getByRole('button',{name:open,exact:true}).waitFor();await page.screenshot({path:`/tmp/macntfs-qa/panel-language-${id}.png`});if(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth||document.documentElement.scrollHeight>innerHeight))throw Error('language panel overflow '+id);
 await page.setViewportSize({width:1060,height:760});await page.goto((process.env.TEST_UI_URL||'http://127.0.0.1:1420'));
 }
 await page.evaluate(()=>{const s=JSON.parse(localStorage.getItem('qa-settings'));s.language='en';localStorage.setItem('qa-settings',JSON.stringify(s));localStorage.setItem('qa-required',JSON.stringify({version:'99.0.0',notes:'test'}));localStorage.setItem('qa-check-error','1')});await page.reload();await page.getByRole('dialog',{name:'Update required',exact:true}).waitFor();
 if(!(await page.locator('main').evaluate(el=>el.inert)))throw Error('mandatory dialog does not isolate disk controls');
 await page.getByRole('dialog').getByRole('button',{name:/Safely eject/}).click();if((await page.evaluate(()=>window.__QA_OPS)).at(-1).action!=='eject')throw Error('mandatory update prevents safe eject');
 await page.evaluate(()=>localStorage.setItem('qa-blockers','1'));await page.reload();await page.getByRole('dialog').waitFor();if(!(await page.getByRole('button',{name:'Update now',exact:true}).isDisabled()))throw Error('update allowed with active mounts');await page.evaluate(()=>localStorage.removeItem('qa-blockers'));await page.reload();await page.getByRole('dialog').waitFor();
 await page.getByRole('button',{name:'Update now',exact:true}).click();await page.getByRole('alert').getByText(/offline/).waitFor();if(!(await page.getByRole('dialog').isVisible()))throw Error('offline cleared mandatory gate');
 await page.evaluate(()=>localStorage.removeItem('qa-check-error'));await page.getByRole('button',{name:'Update now',exact:true}).click();await page.getByRole('alert').getByText(/simulated download failure/).waitFor();await page.getByRole('button',{name:'Update now',exact:true}).click();if((await page.evaluate(()=>window.__QA_WINDOW)).filter(x=>x==='install').length!==2)throw Error('download retry failed');
 await page.reload();await page.getByRole('dialog',{name:'Update required',exact:true}).waitFor();await page.screenshot({path:'/tmp/macntfs-qa/required-update.png'});
 await page.goto((process.env.TEST_UI_URL||'http://127.0.0.1:1420')+'/?panel');await page.locator('.panel-update').waitFor();if(!(await page.getByRole('button',{name:'Enable writing',exact:true}).isDisabled()))throw Error('tray mandatory mount enabled');if(await page.getByRole('button',{name:'Safely eject',exact:true}).isDisabled())throw Error('tray mandatory eject disabled');
 const resolved=await page.evaluate(async()=>{const {resolveLanguage}=await import('/i18n.js');return ['zh-CN','zh-TW','zh-HK','ja-JP','en-US','fr-FR'].map(l=>resolveLanguage('auto',l))});if(resolved.join(',')!=='zh-Hans,zh-Hant,zh-Hant,ja,en,en')throw Error('system language mapping incorrect');
 await page.evaluate(()=>{localStorage.removeItem('qa-required');localStorage.setItem('qa-check-error','1')});await page.goto((process.env.TEST_UI_URL||'http://127.0.0.1:1420'));await page.getByRole('button',{name:'Open Finder',exact:true}).waitFor();if(await page.getByRole('dialog').count())throw Error('unknown offline update locked app');
 if(errors.length)throw Error(errors.join('\n'));console.log('PASS: three themes persist; Finder/eject IPC; settings; native/minimum width; panel buttons/toggle/navigation/multi-volume/busy/error; four languages/persistence; required update/offline/retry/eject; no page errors');await browser.close();
})().catch(e=>{console.error(e);process.exit(1)});

// Evaluate the actual installer selection functions without installing anything.
const fs=require('fs'),vm=require('vm'),assert=require('assert');
const xml=fs.readFileSync('dist/package-build/Distribution','utf8');
const script=xml.match(/<script>([\s\S]*?)<\/script>/)[1].replace(/&lt;/g,'<').replace(/&gt;/g,'>').replace(/&quot;/g,'"').replace(/&apos;/g,"'").replace(/&amp;/g,'&');
function evaluate(os,receipt){
 const context={system:{version:{ProductVersion:os},compareVersions(a,b){const x=a.split('.').map(Number),y=b.split('.').map(Number);for(let i=0;i<Math.max(x.length,y.length);i++){const d=(x[i]||0)-(y[i]||0);if(d)return Math.sign(d)}return 0}},my:{target:{receiptForIdentifier:()=>receipt}},choices:{}};
 vm.createContext(context);vm.runInContext(script,context);
 return {selected:context.defaultStableComponents(),enabled:!context.hasNewerMacFUSE()&&!context.needsStableComponents()};
}
assert.deepEqual(evaluate('13.0',null),{selected:false,enabled:true});
assert.deepEqual(evaluate('27.0',null),{selected:false,enabled:true});
assert.deepEqual(evaluate('27.0',{version:'5.4.0'}),{selected:true,enabled:true});
assert.deepEqual(evaluate('27.0',{version:'5.5.0'}),{selected:false,enabled:false});
assert.deepEqual(evaluate('12.7',null),{selected:true,enabled:false});
assert(xml.includes('customize="always"'));
assert(xml.includes('id="com.macntfs.optional-macfuse"'));
console.log('PASS: fresh kernel-free defaults; optional stable components; existing-driver upgrades; no downgrade; macOS 12 stable requirement');

import {ref,computed} from 'vue';
import messages from './messages.json';
export const languages=['auto','zh-Hans','zh-Hant','en','ja'];
export const locale=ref(localStorage.getItem('macntfs-language')||'auto');
export function resolveLanguage(value,system=navigator.language){
 if(value!=='auto'&&languages.includes(value))return value;
 if(/^zh/i.test(system))return /Hant|TW|HK|MO/i.test(system)?'zh-Hant':'zh-Hans';
 return /^ja/i.test(system)?'ja':'en';
}
export const language=computed(()=>resolveLanguage(locale.value));
export function setLanguage(value){locale.value=languages.includes(value)?value:'auto';localStorage.setItem('macntfs-language',locale.value);document.documentElement.lang=language.value;}
export function tr(key){const mismatch=key?.match(/^应用版本 (.+) 与后台助手版本 (.+) 不一致，请安装对应版本并重新启动应用$/);if(mismatch){return tr('应用版本 {app} 与后台助手版本 {helper} 不一致，请安装对应版本并重新启动应用').replace('{app}',mismatch[1]).replace('{helper}',mismatch[2]);}return language.value==='zh-Hans'?key:messages[key]?.[language.value]||key;}
setLanguage(locale.value);

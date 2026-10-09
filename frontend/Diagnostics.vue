<script setup>
import {ref} from 'vue';
import {invoke} from '@tauri-apps/api/core';
import {tr} from './i18n';
import {Activity,CheckCircle2,AlertCircle,HelpCircle,Download,RefreshCw} from 'lucide-vue-next';
const props=defineProps({busy:Boolean});
const emit=defineEmits(['action']);
const report=ref(null),running=ref(false),failure=ref('');
async function diagnose(){if(running.value)return;running.value=true;failure.value='';try{report.value=await invoke('diagnose')}catch(e){failure.value=String(e)}finally{running.value=false}}
</script>
<template>
<section class="setting-card diagnostics">
<div class="diagnostic-heading"><div><h2><Activity :size="20"/> {{tr('运行诊断')}}</h2><p>{{tr('检查组件、权限与实际挂载状态；不会修改磁盘或上传信息。')}}</p></div><button class="secondary" :disabled="props.busy||running" @click="diagnose"><RefreshCw :size="16"/>{{running?tr('正在检查…'):tr('开始检查')}}</button></div>
<p v-if="failure" role="alert">{{tr(failure)}}</p>
<div v-if="report" class="diagnostic-grid" aria-live="polite">
<article v-for="c in report.checks" :key="c.code" class="diagnostic-check" :data-status="c.status">
<component :is="c.status==='pass'?CheckCircle2:c.status==='error'?AlertCircle:HelpCircle" :size="20"/>
<div><h3>{{tr(c.title)}} <span>{{tr(c.status==='pass'?'已通过':c.status==='error'?'需要处理':'待确认')}}</span></h3><p>{{tr(c.detail)}}</p><button v-if="c.action&&c.status!=='pass'" class="guide-link" :disabled="props.busy" @click="emit('action',c.action)">{{tr(c.action==='release'?'完整安装包':c.action==='permissions'?'磁盘访问权限':'系统授权')}}</button></div>
</article>
</div>
<div v-if="report" class="diagnostic-export"><p>{{tr('报告只包含版本、检查结果和磁盘数量，不包含磁盘名称、UUID、文件路径或网络地址。')}}</p><button class="secondary" :disabled="props.busy||running" @click="emit('action','diagnostics-export')"><Download :size="16"/>{{tr('导出诊断报告')}}</button></div>
</section>
</template>

<script setup>
import {ref,onMounted,onUnmounted,computed} from 'vue';
import {invoke} from '@tauri-apps/api/core';
import {listen} from '@tauri-apps/api/event';
import {FolderOpen,LogOut,HardDrive,ShieldCheck,Power,ArrowUpRight,Settings,RefreshCw} from 'lucide-vue-next';
import driveImage from './assets/drive.png';
const state=ref(null),working=ref(false),failure=ref('');
const locked=computed(()=>working.value||state.value?.monitor.busy);
let timer,stopDevices,stopResult;
async function refresh(){try{state.value=await invoke('snapshot')}catch(e){failure.value=String(e)}}
async function act(action,volume){if(locked.value)return;working.value=true;failure.value='';try{await invoke('operate',{action,volume,backend:state.value.settings.backend})}catch(e){failure.value=String(e)}finally{working.value=false;await refresh()}}
async function toggle(){if(!state.value)return;try{await invoke('set_auto_mount',{enabled:!state.value.settings.auto_mount});await refresh()}catch(e){failure.value=String(e)}}
async function windowAction(action){try{await invoke('panel_action',{action})}catch(e){failure.value=String(e)}}
function escape(e){if(e.key==='Escape')windowAction('hide')}
onMounted(async()=>{await refresh();stopDevices=await listen('devices-changed',refresh);stopResult=await listen('operation-result',()=>{failure.value='';refresh()});timer=setInterval(refresh,3000);window.addEventListener('keydown',escape)});
onUnmounted(()=>{clearInterval(timer);stopDevices?.();stopResult?.();window.removeEventListener('keydown',escape)});
</script>
<template>
<div class="tray-panel shell" :data-theme="state?.settings.theme||'Stone'">
<div class="panel-heading"><img :src="driveImage" alt=""/><div><strong>macntfs</strong><small>磁盘，随时可用。</small></div><button class="panel-icon-button" aria-label="刷新设备" @click="refresh"><RefreshCw :size="16"/></button></div>
<div class="panel-status" :class="{unavailable:!state?.environment.service}"><ShieldCheck :size="16"/><span>{{state?.environment.service?'后台助手已连接 · 挂载无需密码':'后台助手未连接'}}<button v-if="state&&!state.environment.service" @click="windowAction('settings')">设置 <ArrowUpRight :size="12"/></button></span></div>
<div class="panel-devices"><div class="panel-section-label">外置磁盘 <span>{{state?.volumes.length||0}}</span></div>
<article v-for="v in state?.volumes" :key="v.uuid" class="panel-volume"><div class="panel-volume-heading"><img :src="driveImage" alt="外置磁盘"/><div><strong>{{v.name||'未命名磁盘'}}</strong><small>NTFS · {{v.id}}</small></div><span class="panel-badge" :class="{writable:v.writable&&v.mount}">{{v.mount?(v.writable?'可读写':'只读'):'未挂载'}}</span></div><div class="panel-volume-actions"><button :disabled="locked||!v.mount" @click="act('open',v)"><FolderOpen :size="17"/><span>打开 Finder</span></button><button :disabled="locked||!state?.environment.service||(v.writable&&!!v.mount)" @click="act('mount',v)"><HardDrive :size="17"/><span>开启读写</span></button><button :disabled="locked" @click="act('eject',v)" title="安全推出整块磁盘，包括其他分区"><LogOut :size="17"/><span>安全推出</span></button></div></article>
<div v-if="state&&!state.volumes.length" class="panel-empty"><HardDrive :size="30"/><strong>等待连接磁盘</strong><span>插入 NTFS 磁盘后会自动显示。</span></div>
</div>
<div class="panel-auto"><div><strong>插入后自动开启读写</strong><small>仅对状态正常的 NTFS 磁盘启用。</small></div><button class="switch" role="switch" :aria-checked="state?.settings.auto_mount||false" aria-label="插入后自动开启读写" :disabled="!state" @click="toggle"><span></span></button></div>
<div v-if="locked||failure||state?.monitor.last_event" class="panel-result" :class="{error:failure||state?.monitor.last_error}">{{locked?'正在处理磁盘，请稍候…':failure||(state?.monitor.last_event||'').split('\n')[0]}}</div>
<div class="panel-footer"><button @click="windowAction('show')"><ArrowUpRight :size="15"/>打开主窗口</button><button aria-label="设置与更新" @click="windowAction('settings')"><Settings :size="16"/></button><button class="panel-quit" @click="windowAction('quit')"><Power :size="15"/>退出</button></div>
</div>
</template>

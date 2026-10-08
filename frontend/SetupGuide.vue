<script setup>
import {ref,watch} from 'vue';
defineProps({environment:Object,busy:Boolean});
const emit=defineEmits(['action','refresh']);
const saved=Number(localStorage.getItem('macntfs-setup-step'));
const step=ref(saved>=1&&saved<=4?saved:1);
watch(step,value=>localStorage.setItem('macntfs-setup-step',String(value)));
const steps=['安装应用','允许驱动','磁盘访问','完成检查'];
</script>
<template>
<section class="setting-card guide">
<h2>首次使用向导 <span class="version">按顺序完成，只需设置一次</span></h2>
<p>安装授权、驱动授权、磁盘访问是三个不同步骤。你只需按当前页面操作；软件不会替你更改启动安全策略。</p>
<div class="guide-tabs"><button v-for="(title,index) in steps" :key="title" :class="{selected:step===index+1}" @click="step=index+1">{{index+1}} · {{title}}</button></div>
<div v-if="step===1">
<h3>1 · 安装到「应用程序」</h3><p>打开下载的完整 PKG，按「继续 → 安装」完成系统授权。安装时输入 Mac 登录密码；如果系统显示 Touch ID，也可使用指纹。安装完成后，从 Finder 的「应用程序」打开 macntfs。</p>
<p>更新时也使用完整安装包，同时更新应用和助手。安装前从菜单栏选择「退出 macntfs」，避免旧版本继续运行。</p>
<div class="guide-status">{{environment?.runtime?'读写组件版本检查通过':'读写组件尚未安装或版本不一致'}} · {{environment?.fuse?'macFUSE 已安装':'macFUSE 尚未安装'}}</div>
<button class="secondary" :disabled="busy" @click="emit('action','install')">打开本地组件安装器</button>
</div>
<div v-else-if="step===2">
<h3>2 · 允许 macFUSE 驱动</h3><p>默认「稳定模式」使用 macFUSE 内核驱动。首次使用或更新 macFUSE 后，若系统提示扩展被阻止，打开「系统设置 → 隐私与安全性」，找到 macFUSE / Benjamin Fleischer 的提示并选择「允许」，按提示验证并重启。</p>
<button class="secondary" :disabled="busy" @click="emit('action','settings')">打开系统授权设置</button>
<details><summary>Apple 芯片 Mac：系统要求启用扩展或进入恢复模式时，具体怎么做？</summary>
<p>适用于 M1、M2、M3 等 Apple 芯片，且你选择了稳定内核模式。驱动已正常工作时可跳过，不需要每次重启。调整为「降低安全性」会放宽对第三方内核扩展的限制，请了解后由你自行决定。</p>
<ol><li>保存工作，关闭 Mac。完全关机后，长按电源键 / Touch ID 按钮，直到出现「正在载入启动选项」。</li><li>选择齿轮图标「选项 → 继续」，按提示选择管理员并输入密码。</li><li>在恢复环境顶部菜单选择「实用工具 → 启动安全性实用工具」。若它自动打开，直接继续。</li><li>选择安装 macOS 的启动磁盘（通常是 Macintosh HD），点击「安全策略」。</li><li>选择「降低安全性」，勾选「允许用户管理来自被认可开发者的内核扩展」（不同系统版本翻译可能略有差异），确认并输入密码。</li><li>重新启动回到 macOS，打开「系统设置 → 隐私与安全性」，允许 macFUSE / Benjamin Fleischer 的扩展；如系统要求，再重启一次。</li><li>回到 macntfs，点「重新检查」，再连接磁盘。</li></ol>
<p>不需要关闭 SIP、关闭 FileVault 或勾选远程管理内核扩展。如果是公司管理的 Mac，交由管理员配置。</p>
</details>
<details><summary>Intel Mac 或不想调整启动安全策略？</summary><p>Intel Mac 不使用上述 Apple 芯片恢复模式步骤，一般在系统设置允许驱动并重启。macOS 15.4 及以上的 FSKit 后端不要求内核扩展或恢复模式，但本软件的 FSKit 尚属实验性，不能保证与稳定模式同等可用；可在下方读写引擎中选择。</p></details>
<p class="guide-note">是否允许驱动、是否需要重启由 macOS 判断，本向导不能自动读取完整授权状态。</p>
</div>
<div v-else-if="step===3">
<h3>3 · 允许助手访问外置磁盘</h3><p>如果挂载提示「macOS 拒绝访问磁盘设备」，按下面步骤设置完整磁盘访问。这项权限范围较广，请只添加本软件安装的 ntfs-helper。</p>
<ol><li>点「打开完整磁盘访问」，在系统设置解锁或完成身份验证。</li><li>点列表中的「＋」。在文件选择窗口按 Command + Shift + G，粘贴下方路径并回车，再点「打开」。</li><li>确认 ntfs-helper 的开关已开启。回到 macntfs 重新检查并重试。</li></ol>
<code class="helper-path">/Library/Application Support/NTFS Desktop/Runtime/bin/ntfs-helper</code>
<div class="actions"><button class="secondary" :disabled="busy" @click="emit('action','permissions')">打开完整磁盘访问</button><button class="secondary" :disabled="busy" @click="emit('action','permission-helper')">在 Finder 定位助手</button></div>
<p>当前开发测试包采用临时签名，更新助手后若旧授权失效：在列表移除旧 ntfs-helper，再添加上面的当前助手。仅勾选主应用不能代替助手的磁盘权限。</p>
</div>
<div v-else>
<h3>4 · 检查并开始使用</h3><div class="guide-status">{{environment?.service?'✓ 后台助手已连接，日常挂载无需密码':'后台助手尚未连接'}}<p v-if="!environment?.service">{{environment?.service_issue}}</p></div>
<p>连接 NTFS 磁盘，回到「我的磁盘」确认显示「可读写」。助手连接成功不代表已经取得所有磁盘权限；以实际挂载结果为准。Windows 休眠或损坏的磁盘不会强行开启读写。</p>
<button class="primary" :disabled="busy" @click="emit('refresh')">重新检查</button>
</div>
<div class="guide-footer"><button class="secondary" v-if="step>1" @click="step--">上一步</button><button class="primary" v-if="step<4" @click="step++">下一步</button></div>
<p class="guide-note">官方说明：<button class="guide-link" @click="emit('action','guide-macfuse')">macFUSE 安装指南</button> · <button class="guide-link" @click="emit('action','guide-apple')">Apple 启动安全策略</button></p>
</section>
</template>

import {createApp} from 'vue';
import App from './App.vue';
import TrayPanel from './TrayPanel.vue';
import './style.css';
const isPanel=new URLSearchParams(location.search).has('panel');
document.documentElement.classList.toggle('tray-document',isPanel);
createApp(isPanel?TrayPanel:App).mount('#app');

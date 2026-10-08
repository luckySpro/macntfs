import {createApp} from 'vue';
import App from './App.vue';
import TrayPanel from './TrayPanel.vue';
import './style.css';
createApp(new URLSearchParams(location.search).has('panel')?TrayPanel:App).mount('#app');

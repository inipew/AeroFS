import { createApp } from 'vue';
import { createPinia } from 'pinia';
import { VueQueryPlugin } from '@tanstack/vue-query';
import { queryClient } from './queryClient';
import './style.css';
import App from './App.vue';

import { realtimeSync } from './services/realtimeSync';

const app = createApp(App);
const pinia = createPinia();

realtimeSync.start();

app.use(pinia);
app.use(VueQueryPlugin, { queryClient });
app.mount('#app');

import { createApp } from 'vue';
import { createPinia } from 'pinia';
import { VueQueryPlugin, QueryClient } from '@tanstack/vue-query';
import './style.css';
import App from './App.vue';

const app = createApp(App);
const pinia = createPinia();

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,        // 30 s before background refetch
      gcTime: 120_000,          // 2 min cache retention after unmount
      refetchOnWindowFocus: false,
      retry: 1,
    },
  },
});

app.use(pinia);
app.use(VueQueryPlugin, { queryClient });
app.mount('#app');


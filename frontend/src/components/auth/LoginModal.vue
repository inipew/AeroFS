<template>
  <div class="fixed inset-0 z-50 bg-slate-950/95 backdrop-blur-xl flex items-center justify-center p-4 select-none font-sans">
    <!-- Login Card -->
    <div class="bg-slate-900 border border-slate-800 rounded-3xl max-w-sm w-full p-8 shadow-2xl relative overflow-hidden ring-1 ring-white/10">
      <!-- Glow background decoration -->
      <div class="absolute -top-20 -right-20 w-40 h-40 bg-blue-600/20 rounded-full blur-3xl pointer-events-none"></div>
      <div class="absolute -bottom-20 -left-20 w-40 h-40 bg-indigo-600/20 rounded-full blur-3xl pointer-events-none"></div>

      <!-- Brand Logo -->
      <div class="flex flex-col items-center mb-6 text-center relative z-10">
        <div class="w-13 h-13 rounded-2xl bg-gradient-to-tr from-blue-600 to-indigo-500 flex items-center justify-center font-bold text-white text-2xl shadow-xl ring-2 ring-white/10 mb-3.5">
          <svg class="w-7 h-7" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"></path>
          </svg>
        </div>
        <h2 class="text-xl font-bold text-white tracking-tight">FileBrowser</h2>
        <p class="text-slate-400 text-xs mt-1">Multi-Backend Cloud Storage Gateway</p>
      </div>

      <!-- Error Alert -->
      <div v-if="authStore.error" class="mb-4 p-3 bg-red-500/10 border border-red-500/30 rounded-xl text-red-300 text-xs flex items-center space-x-2">
        <span>⚠️</span>
        <span>{{ authStore.error }}</span>
      </div>

      <!-- Login Form -->
      <form @submit.prevent="handleLogin" class="space-y-4 relative z-10">
        <div>
          <label class="block text-slate-400 text-[11px] font-semibold uppercase tracking-wider mb-1.5">Username</label>
          <input
            v-model="username"
            type="text"
            placeholder="admin"
            class="w-full bg-slate-950 border border-slate-800 rounded-xl px-3.5 py-2.5 text-white placeholder-slate-600 focus:outline-none focus:border-blue-500 transition text-xs shadow-inner"
            required
          />
        </div>

        <div>
          <label class="block text-slate-400 text-[11px] font-semibold uppercase tracking-wider mb-1.5">Password</label>
          <input
            v-model="password"
            type="password"
            placeholder="••••••••"
            class="w-full bg-slate-950 border border-slate-800 rounded-xl px-3.5 py-2.5 text-white placeholder-slate-600 focus:outline-none focus:border-blue-500 transition text-xs shadow-inner"
            required
          />
        </div>

        <button
          type="submit"
          :disabled="loading"
          class="w-full py-2.5 rounded-xl bg-blue-600 hover:bg-blue-500 text-white font-semibold shadow-lg transition duration-150 disabled:opacity-50 text-xs flex items-center justify-center space-x-2 cursor-pointer mt-2"
        >
          <span v-if="loading" class="animate-spin rounded-full h-3.5 w-3.5 border-2 border-white border-t-transparent"></span>
          <span>{{ loading ? 'Signing in...' : 'Sign In' }}</span>
        </button>
      </form>

      <!-- Default Credentials Hint -->
      <div class="mt-6 pt-4 border-t border-slate-800/80 text-center text-[11px] text-slate-500">
        <p>Demo credentials: <span class="font-mono text-slate-300">admin</span> / <span class="font-mono text-slate-300">admin12345</span></p>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { useAuthStore } from '../../stores/authStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useTransferStore } from '../../stores/transferStore';

const authStore = useAuthStore();
const workspaceStore = useWorkspaceStore();
const transferStore = useTransferStore();

const username = ref('admin');
const password = ref('admin12345');
const loading = ref(false);

async function handleLogin() {
  loading.value = true;
  const ok = await authStore.login({
    username: username.value,
    password: password.value,
  });
  if (ok) {
    transferStore.connectWs();
    await transferStore.fetchJobs();
    await workspaceStore.fetchPanelEntries('left');
    if (workspaceStore.isDualPane) {
      await workspaceStore.fetchPanelEntries('right');
    }
  }
  loading.value = false;
}
</script>

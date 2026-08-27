<template>
  <div
    v-if="isOpen"
    class="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4 select-none font-sans text-xs animate-in fade-in duration-150"
  >
    <div class="bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-800 rounded-3xl max-w-md w-full p-6 shadow-2xl relative">
      <div class="flex items-center space-x-3 mb-4">
        <div class="w-10 h-10 rounded-xl bg-blue-600/10 dark:bg-blue-500/20 text-blue-600 dark:text-blue-400 flex items-center justify-center">
          <FbIcon name="folder" size="20px" />
        </div>
        <div>
          <h3 class="text-base font-bold text-gray-900 dark:text-white">Add Storage Source</h3>
          <p class="text-gray-500 dark:text-slate-400 text-xs">Connect to FTP, FTPS, or SFTP servers.</p>
        </div>
      </div>

      <form @submit.prevent="handleSave" class="space-y-3.5">
        <!-- Display Name -->
        <div>
          <label class="block text-gray-700 dark:text-slate-300 text-[11px] font-semibold mb-1">Display Name</label>
          <input
            v-model="form.name"
            type="text"
            placeholder="e.g. My FTP Server"
            class="w-full bg-gray-50 dark:bg-slate-950 border border-gray-200 dark:border-slate-800 rounded-xl px-3.5 py-2 text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:border-blue-500 text-xs shadow-inner"
            required
          />
        </div>

        <!-- Protocol / Provider -->
        <div>
          <label class="block text-gray-700 dark:text-slate-300 text-[11px] font-semibold mb-1">Protocol / Provider</label>
          <select
            v-model="form.provider"
            class="w-full bg-gray-50 dark:bg-slate-950 border border-gray-200 dark:border-slate-800 rounded-xl px-3.5 py-2 text-gray-900 dark:text-white focus:outline-none focus:border-blue-500 text-xs cursor-pointer shadow-inner"
          >
            <option value="ftp">FTP (Standard File Transfer Protocol - Port 21)</option>
            <option value="ftps">FTPS (Explicit/Implicit TLS - Port 990/21)</option>
            <option value="sftp">SFTP (SSH File Transfer Protocol - Port 22)</option>
          </select>
        </div>

        <!-- Host & Port -->
        <div class="grid grid-cols-3 gap-2">
          <div class="col-span-2">
            <label class="block text-gray-700 dark:text-slate-300 text-[11px] font-semibold mb-1">Host / IP</label>
            <input
              v-model="form.host"
              type="text"
              placeholder="192.168.1.100 or ftp.example.com"
              class="w-full bg-gray-50 dark:bg-slate-950 border border-gray-200 dark:border-slate-800 rounded-xl px-3.5 py-2 text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:border-blue-500 text-xs shadow-inner"
              required
            />
          </div>
          <div>
            <label class="block text-gray-700 dark:text-slate-300 text-[11px] font-semibold mb-1">Port</label>
            <input
              v-model.number="form.port"
              type="number"
              class="w-full bg-gray-50 dark:bg-slate-950 border border-gray-200 dark:border-slate-800 rounded-xl px-3.5 py-2 text-gray-900 dark:text-white focus:outline-none focus:border-blue-500 text-xs shadow-inner"
              required
            />
          </div>
        </div>

        <!-- Username -->
        <div>
          <label class="block text-gray-700 dark:text-slate-300 text-[11px] font-semibold mb-1">Username</label>
          <input
            v-model="form.username"
            type="text"
            placeholder="anonymous or ftpuser"
            class="w-full bg-gray-50 dark:bg-slate-950 border border-gray-200 dark:border-slate-800 rounded-xl px-3.5 py-2 text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:border-blue-500 text-xs shadow-inner"
            required
          />
        </div>

        <!-- Password / Secret -->
        <div>
          <label class="block text-gray-700 dark:text-slate-300 text-[11px] font-semibold mb-1">Password or Private Key</label>
          <input
            v-model="form.secret"
            type="password"
            placeholder="•••••••• (Encrypted in Vault)"
            class="w-full bg-gray-50 dark:bg-slate-950 border border-gray-200 dark:border-slate-800 rounded-xl px-3.5 py-2 text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:border-blue-500 text-xs shadow-inner"
          />
        </div>

        <!-- Remote Base Path -->
        <div>
          <label class="block text-gray-700 dark:text-slate-300 text-[11px] font-semibold mb-1">Remote Base Path</label>
          <input
            v-model="form.base_path"
            type="text"
            placeholder="/ or /public_html"
            class="w-full bg-gray-50 dark:bg-slate-950 border border-gray-200 dark:border-slate-800 rounded-xl px-3.5 py-2 text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:border-blue-500 text-xs shadow-inner"
          />
        </div>

        <!-- Action Buttons -->
        <div class="flex justify-between items-center pt-3 border-t border-gray-100 dark:border-slate-800">
          <button
            type="button"
            @click="isOpen = false"
            class="px-4 py-2 rounded-xl text-gray-600 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800 transition font-medium text-xs cursor-pointer"
          >
            Cancel
          </button>
          
          <button
            type="submit"
            :disabled="saving"
            class="px-5 py-2 rounded-xl bg-blue-600 hover:bg-blue-700 text-white font-semibold shadow-xs transition disabled:opacity-50 text-xs cursor-pointer flex items-center space-x-1.5"
          >
            <span v-if="saving" class="animate-spin rounded-full h-3 w-3 border-2 border-white border-t-transparent"></span>
            <span>{{ saving ? 'Connecting...' : 'Save & Connect' }}</span>
          </button>
        </div>
      </form>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { apiClient } from '../../api/client';
import { useConnectionStore } from '../../stores/connectionStore';
import { useUiStore } from '../../stores/uiStore';
import type { ProviderKind } from '../../types/connection';

const props = defineProps<{
  modelValue: boolean;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
}>();

const connStore = useConnectionStore();
const uiStore = useUiStore();

const isOpen = ref(props.modelValue);
const saving = ref(false);

watch(
  () => props.modelValue,
  (val) => {
    isOpen.value = val;
  }
);

watch(
  () => isOpen.value,
  (val) => {
    emit('update:modelValue', val);
  }
);

const form = ref<{
  name: string;
  provider: ProviderKind;
  host: string;
  port: number;
  username: string;
  secret: string;
  base_path: string;
}>({
  name: '',
  provider: 'ftp',
  host: '',
  port: 21,
  username: '',
  secret: '',
  base_path: '/',
});

watch(
  () => form.value.provider,
  (prov) => {
    if (prov === 'ftp') form.value.port = 21;
    if (prov === 'ftps') form.value.port = 990;
    if (prov === 'sftp') form.value.port = 22;
  }
);

async function handleSave() {
  saving.value = true;
  try {
    const resp = await apiClient.post('/connections', form.value);
    uiStore.showToast(resp.data.message || 'Connection created!', 'success');
    
    // Refresh connections list
    const connsResp = await apiClient.get('/connections');
    connStore.connections = connsResp.data;

    isOpen.value = false;
  } catch (err: any) {
    uiStore.showToast(err.response?.data?.error?.message || 'Failed to save connection', 'error');
  } finally {
    saving.value = false;
  }
}
</script>

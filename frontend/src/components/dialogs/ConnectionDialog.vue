<template>
  <Transition name="ios-modal">
    <div
      v-if="isOpen"
      class="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4 select-none font-sans text-xs"
      @click="isOpen = false"
    >
      <div class="modal-card bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-800 rounded-3xl max-w-md w-full p-6 shadow-2xl relative" @click.stop>
      <div class="flex items-center space-x-3 mb-4">
        <div class="w-10 h-10 rounded-xl bg-blue-600/10 dark:bg-blue-500/20 text-blue-600 dark:text-blue-400 flex items-center justify-center">
          <FbIcon :name="isEditing ? 'rename' : 'folder'" size="20px" />
        </div>
        <div>
          <h3 class="text-base font-bold text-gray-900 dark:text-white">
            {{ isEditing ? 'Edit Storage Source' : 'Add Storage Source' }}
          </h3>
          <p class="text-gray-500 dark:text-slate-400 text-xs">
            {{ isEditing ? `Update configuration for ${props.connectionToEdit?.name || 'storage'}.` : 'Connect to FTP, FTPS, SFTP, or S3 storage.' }}
          </p>
        </div>
      </div>

      <form @submit.prevent="handleSave" class="space-y-3.5">
        <!-- Display Name -->
        <div>
          <label class="block text-gray-700 dark:text-slate-300 text-[11px] font-semibold mb-1">Display Name</label>
          <input
            v-model="form.name"
            type="text"
            placeholder="e.g. My Storage Server"
            class="w-full bg-gray-50 dark:bg-slate-950 border border-gray-200 dark:border-slate-800 rounded-xl px-3.5 py-2 text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:border-blue-500 text-xs shadow-inner"
            required
          />
        </div>

        <!-- Protocol / Provider -->
        <div>
          <label class="block text-gray-700 dark:text-slate-300 text-[11px] font-semibold mb-1">
            Protocol / Provider <span v-if="isEditing" class="text-gray-400 font-normal">(cannot be changed)</span>
          </label>
          <select
            v-model="form.provider"
            :disabled="isEditing"
            class="w-full bg-gray-50 dark:bg-slate-950 border border-gray-200 dark:border-slate-800 rounded-xl px-3.5 py-2 text-gray-900 dark:text-white focus:outline-none focus:border-blue-500 text-xs shadow-inner disabled:opacity-60 disabled:cursor-not-allowed cursor-pointer"
          >
            <option value="ftp">FTP (Standard File Transfer Protocol - Port 21)</option>
            <option value="ftps">FTPS (Explicit/Implicit TLS - Port 990/21)</option>
            <option value="sftp">SFTP (SSH File Transfer Protocol - Port 22)</option>
            <option value="s3">S3 / Object Storage (Amazon S3, MinIO, Wasabi, etc.)</option>
          </select>
        </div>

        <!-- Host & Port or S3 Bucket -->
        <div v-if="form.provider === 's3'">
          <label class="block text-gray-700 dark:text-slate-300 text-[11px] font-semibold mb-1">Bucket Name</label>
          <input
            v-model="form.host"
            type="text"
            placeholder="e.g. my-s3-bucket"
            class="w-full bg-gray-50 dark:bg-slate-950 border border-gray-200 dark:border-slate-800 rounded-xl px-3.5 py-2 text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:border-blue-500 text-xs shadow-inner"
            required
          />
        </div>
        <div v-else class="grid grid-cols-3 gap-2">
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

        <!-- Username / Access Key ID -->
        <div>
          <label class="block text-gray-700 dark:text-slate-300 text-[11px] font-semibold mb-1">
            {{ form.provider === 's3' ? 'Access Key ID' : 'Username' }}
          </label>
          <input
            v-model="form.username"
            type="text"
            :placeholder="form.provider === 's3' ? 'AKIA... or MinIO access key' : 'anonymous or ftpuser'"
            class="w-full bg-gray-50 dark:bg-slate-950 border border-gray-200 dark:border-slate-800 rounded-xl px-3.5 py-2 text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:border-blue-500 text-xs shadow-inner"
          />
        </div>

        <!-- Password / Secret Access Key -->
        <div>
          <label class="block text-gray-700 dark:text-slate-300 text-[11px] font-semibold mb-1">
            {{ form.provider === 's3' ? 'Secret Access Key' : 'Password or Private Key' }}
          </label>
          <input
            v-model="form.secret"
            type="password"
            :placeholder="isEditing ? '•••••••• (Leave blank to keep current credentials)' : '•••••••• (Encrypted in Vault)'"
            class="w-full bg-gray-50 dark:bg-slate-950 border border-gray-200 dark:border-slate-800 rounded-xl px-3.5 py-2 text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:border-blue-500 text-xs shadow-inner"
          />
        </div>

        <!-- Remote Base Path / Bucket Prefix -->
        <div>
          <label class="block text-gray-700 dark:text-slate-300 text-[11px] font-semibold mb-1">
            {{ form.provider === 's3' ? 'Bucket Prefix / Root Path' : 'Remote Base Path' }}
          </label>
          <input
            v-model="form.base_path"
            type="text"
            :placeholder="form.provider === 's3' ? '/ or /data' : '/ or /public_html'"
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
            <span>{{ saving ? (isEditing ? 'Saving...' : 'Connecting...') : (isEditing ? 'Save Changes' : 'Save & Connect') }}</span>
          </button>
        </div>
      </form>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { createConnectionApi, updateConnectionApi, listConnectionsApi } from '../../api/connections';
import { queryClient } from '../../queryClient';
import { queryKeys } from '../../api/queryKeys';
import { normalizeApiError } from '../../utils/errorNormalizer';
import { useConnectionStore } from '../../stores/connectionStore';
import { useUiStore } from '../../stores/uiStore';
import type { Connection, ProviderKind } from '../../types/connection';

const props = defineProps<{
  modelValue: boolean;
  connectionToEdit?: Connection | null;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
}>();

const connStore = useConnectionStore();
const uiStore = useUiStore();

const isOpen = ref(props.modelValue);
const saving = ref(false);
const isEditing = computed(() => !!props.connectionToEdit);

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

function defaultPortFor(provider: ProviderKind): number {
  if (provider === 'ftp') return 21;
  if (provider === 'ftps') return 990;
  if (provider === 'sftp') return 22;
  if (provider === 's3') return 443;
  return 21;
}

function syncFormWithEdit() {
  if (props.connectionToEdit) {
    form.value = {
      name: props.connectionToEdit.name,
      provider: props.connectionToEdit.provider,
      host: props.connectionToEdit.host || '',
      port: props.connectionToEdit.port || defaultPortFor(props.connectionToEdit.provider),
      username: props.connectionToEdit.username || '',
      secret: '',
      base_path: props.connectionToEdit.base_path || '/',
    };
  } else {
    form.value = {
      name: '',
      provider: 'ftp',
      host: '',
      port: 21,
      username: '',
      secret: '',
      base_path: '/',
    };
  }
}

watch(
  () => props.modelValue,
  (val) => {
    isOpen.value = val;
    if (val) {
      syncFormWithEdit();
    }
  }
);

watch(
  () => props.connectionToEdit,
  () => {
    if (isOpen.value) {
      syncFormWithEdit();
    }
  }
);

watch(
  () => isOpen.value,
  (val) => {
    emit('update:modelValue', val);
  }
);

watch(
  () => form.value.provider,
  (prov) => {
    if (!isEditing.value) {
      form.value.port = defaultPortFor(prov);
    }
  }
);

async function handleSave() {
  saving.value = true;
  try {
    if (isEditing.value && props.connectionToEdit) {
      const payload: Record<string, any> = {
        name: form.value.name,
        host: form.value.host,
        port: form.value.port,
        username: form.value.username,
        base_path: form.value.base_path,
      };
      if (form.value.secret && form.value.secret.trim().length > 0) {
        payload.secret = form.value.secret.trim();
      }
      const data = await updateConnectionApi(props.connectionToEdit.id, payload);
      uiStore.showToast(data.message || 'Connection updated!', 'success');
      queryClient.invalidateQueries({ queryKey: queryKeys.connections() });
      queryClient.invalidateQueries({ queryKey: queryKeys.connection(props.connectionToEdit.id) });
    } else {
      const data = await createConnectionApi(form.value);
      uiStore.showToast(data.message || 'Connection created!', 'success');
      queryClient.invalidateQueries({ queryKey: queryKeys.connections() });
    }
    
    // Refresh connections list in pinia store
    const conns = await listConnectionsApi();
    connStore.connections = conns;

    isOpen.value = false;
  } catch (err: unknown) {
    uiStore.showToast(normalizeApiError(err).message, 'error');
  } finally {
    saving.value = false;
  }
}
</script>

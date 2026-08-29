<template>
  <Transition name="ios-modal">
    <div
      v-if="uiStore.isSyncOpen"
      class="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4 select-none font-sans text-xs"
      @click="uiStore.isSyncOpen = false"
    >
      <div
        class="modal-card bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-800 rounded-3xl max-w-md w-full p-6 shadow-2xl space-y-4"
        @click.stop
      >
        <div class="flex items-center space-x-3">
          <div class="w-10 h-10 rounded-2xl bg-indigo-600/10 dark:bg-indigo-500/20 text-indigo-600 dark:text-indigo-400 flex items-center justify-center text-lg shadow-xs">
            🔄
          </div>
          <div>
            <h3 class="text-sm font-bold text-gray-900 dark:text-white">Directory Synchronization</h3>
            <p class="text-gray-500 dark:text-slate-400 text-xs">
              Reconcile files between two storage endpoints.
            </p>
          </div>
        </div>

        <form @submit.prevent="handleStartSync" class="space-y-3.5">
          <!-- Source Section -->
          <div class="p-3 bg-gray-50/80 dark:bg-slate-950/60 rounded-2xl border border-gray-100 dark:border-slate-800/80 space-y-2">
            <span class="text-[10px] font-bold uppercase tracking-wider text-blue-600 dark:text-blue-400">Source</span>
            <div class="grid grid-cols-1 sm:grid-cols-2 gap-2">
              <div>
                <label class="block text-gray-700 dark:text-slate-300 text-[11px] font-medium mb-1">Connection</label>
                <select
                  v-model="sourceConn"
                  class="w-full bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-800 rounded-xl px-2.5 py-1.5 text-gray-900 dark:text-white text-xs focus:outline-none focus:border-blue-500 cursor-pointer shadow-xs"
                >
                  <option value="local">Local Storage</option>
                  <option
                    v-for="conn in connectionStore.connections"
                    :key="conn.id"
                    :value="conn.id"
                  >
                    {{ conn.name }} ({{ conn.provider_type }})
                  </option>
                </select>
              </div>
              <div>
                <label class="block text-gray-700 dark:text-slate-300 text-[11px] font-medium mb-1">Path</label>
                <input
                  v-model="sourcePath"
                  type="text"
                  placeholder="/"
                  class="w-full bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-800 rounded-xl px-2.5 py-1.5 text-gray-900 dark:text-white text-xs focus:outline-none focus:border-blue-500 shadow-xs"
                  required
                />
              </div>
            </div>
          </div>

          <!-- Destination Section -->
          <div class="p-3 bg-gray-50/80 dark:bg-slate-950/60 rounded-2xl border border-gray-100 dark:border-slate-800/80 space-y-2">
            <span class="text-[10px] font-bold uppercase tracking-wider text-emerald-600 dark:text-emerald-400">Destination</span>
            <div class="grid grid-cols-1 sm:grid-cols-2 gap-2">
              <div>
                <label class="block text-gray-700 dark:text-slate-300 text-[11px] font-medium mb-1">Connection</label>
                <select
                  v-model="destConn"
                  class="w-full bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-800 rounded-xl px-2.5 py-1.5 text-gray-900 dark:text-white text-xs focus:outline-none focus:border-blue-500 cursor-pointer shadow-xs"
                >
                  <option value="local">Local Storage</option>
                  <option
                    v-for="conn in connectionStore.connections"
                    :key="conn.id"
                    :value="conn.id"
                  >
                    {{ conn.name }} ({{ conn.provider_type }})
                  </option>
                </select>
              </div>
              <div>
                <label class="block text-gray-700 dark:text-slate-300 text-[11px] font-medium mb-1">Path</label>
                <input
                  v-model="destPath"
                  type="text"
                  placeholder="/"
                  class="w-full bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-800 rounded-xl px-2.5 py-1.5 text-gray-900 dark:text-white text-xs focus:outline-none focus:border-blue-500 shadow-xs"
                  required
                />
              </div>
            </div>
          </div>

          <!-- Strategy Section -->
          <div>
            <label class="block text-gray-700 dark:text-slate-300 text-[11px] font-semibold mb-1">Reconciliation Strategy</label>
            <select
              v-model="strategy"
              class="w-full bg-gray-50 dark:bg-slate-950 border border-gray-200 dark:border-slate-800 rounded-xl px-3 py-2 text-gray-900 dark:text-white text-xs focus:outline-none focus:border-blue-500 cursor-pointer shadow-xs"
            >
              <option value="keep_both">Keep Both (Create conflict copy with timestamp)</option>
              <option value="source_wins">Source Wins (Overwrite destination with source file)</option>
              <option value="dest_wins">Destination Wins (Preserve destination copy)</option>
              <option value="newest_wins">Newest Wins (Compare timestamps, newest wins)</option>
            </select>
          </div>

          <!-- Action Buttons -->
          <div class="flex justify-end space-x-2 pt-2 border-t border-gray-100 dark:border-slate-800">
            <button
              type="button"
              @click="uiStore.isSyncOpen = false"
              class="px-4 py-2 rounded-xl text-gray-600 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800 transition font-medium text-xs cursor-pointer"
            >
              Cancel
            </button>
            <button
              type="submit"
              :disabled="loading"
              class="px-5 py-2 rounded-xl bg-indigo-600 hover:bg-indigo-700 text-white font-semibold shadow-xs transition disabled:opacity-50 text-xs cursor-pointer flex items-center space-x-1.5"
            >
              <span v-if="loading" class="animate-spin rounded-full h-3 w-3 border-2 border-white border-t-transparent"></span>
              <span>{{ loading ? 'Starting Sync...' : 'Start Sync' }}</span>
            </button>
          </div>
        </form>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import { useUiStore } from '../../stores/uiStore';
import { useConnectionStore } from '../../stores/connectionStore';
import { useTransferStore } from '../../stores/transferStore';
import { createSyncJobApi } from '../../api/sync';
import type { SyncStrategy } from '../../types/sync';

const uiStore = useUiStore();
const connectionStore = useConnectionStore();
const transferStore = useTransferStore();

const sourceConn = ref<string>('local');
const sourcePath = ref<string>('/');
const destConn = ref<string>('local');
const destPath = ref<string>('/');
const strategy = ref<SyncStrategy>('keep_both');
const loading = ref<boolean>(false);

watch(
  () => uiStore.isSyncOpen,
  (open) => {
    if (open) {
      sourceConn.value = uiStore.syncSourceConnection || 'local';
      sourcePath.value = uiStore.syncSourcePath || '/';
      destConn.value = uiStore.syncDestConnection || 'local';
      destPath.value = uiStore.syncDestPath || '/';
      strategy.value = 'keep_both';
    }
  }
);

async function handleStartSync() {
  loading.value = true;
  try {
    const res = await createSyncJobApi({
      source_connection_id: sourceConn.value,
      source_path: sourcePath.value,
      destination_connection_id: destConn.value,
      destination_path: destPath.value,
      strategy: strategy.value,
    });

    uiStore.showToast(res.message || 'Sync job initiated successfully', 'success');
    uiStore.isSyncOpen = false;
    transferStore.isDrawerOpen = true;
  } catch (err: any) {
    const msg = err.response?.data?.error?.message || err.message || 'Failed to start sync';
    uiStore.showToast(msg, 'error');
  } finally {
    loading.value = false;
  }
}
</script>

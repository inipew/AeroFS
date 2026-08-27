<template>
  <div
    v-if="uiStore.isRenameOpen"
    class="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4 select-none font-sans text-xs animate-in fade-in duration-150"
  >
    <div class="bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-800 rounded-3xl max-w-sm w-full p-6 shadow-2xl">
      <div class="flex items-center space-x-3 mb-3">
        <div class="w-10 h-10 rounded-2xl bg-blue-600/10 dark:bg-blue-500/20 text-blue-600 dark:text-blue-400 flex items-center justify-center">
          <FbIcon name="rename" size="20px" />
        </div>
        <div>
          <h3 class="text-sm font-bold text-gray-900 dark:text-white">Rename Item</h3>
          <p class="text-gray-500 dark:text-slate-400 text-xs font-mono truncate max-w-[200px]">
            {{ uiStore.renameTarget?.name }}
          </p>
        </div>
      </div>

      <form @submit.prevent="handleSubmit" class="space-y-4">
        <div>
          <label class="block text-gray-700 dark:text-slate-300 text-[11px] font-semibold mb-1">New Name</label>
          <input
            ref="inputRef"
            v-model="newName"
            type="text"
            class="w-full bg-gray-50 dark:bg-slate-950 border border-gray-200 dark:border-slate-800 rounded-xl px-3.5 py-2 text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:border-blue-500 text-xs shadow-inner"
            required
          />
        </div>

        <div class="flex justify-end space-x-2 pt-2 border-t border-gray-100 dark:border-slate-800">
          <button
            type="button"
            @click="uiStore.isRenameOpen = false"
            class="px-4 py-2 rounded-xl text-gray-600 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800 transition font-medium text-xs cursor-pointer"
          >
            Cancel
          </button>
          <button
            type="submit"
            :disabled="loading || !newName.trim() || newName === uiStore.renameTarget?.name"
            class="px-5 py-2 rounded-xl bg-blue-600 hover:bg-blue-700 text-white font-semibold shadow-xs transition disabled:opacity-50 text-xs cursor-pointer flex items-center space-x-1.5"
          >
            <span v-if="loading" class="animate-spin rounded-full h-3 w-3 border-2 border-white border-t-transparent"></span>
            <span>{{ loading ? 'Renaming...' : 'Rename' }}</span>
          </button>
        </div>
      </form>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, nextTick } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { renameEntryApi } from '../../api/files';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useUiStore } from '../../stores/uiStore';

const workspaceStore = useWorkspaceStore();
const uiStore = useUiStore();

const newName = ref('');
const loading = ref(false);
const inputRef = ref<HTMLInputElement | null>(null);

watch(
  () => uiStore.isRenameOpen,
  (open) => {
    if (open && uiStore.renameTarget) {
      newName.value = uiStore.renameTarget.name;
      nextTick(() => {
        inputRef.value?.focus();
        inputRef.value?.select();
      });
    }
  }
);

async function handleSubmit() {
  if (!uiStore.renameTarget || !newName.value.trim()) return;
  loading.value = true;
  try {
    const activeP = workspaceStore.getPanel(workspaceStore.activePanelId);
    const oldPath = uiStore.renameTarget.path;
    const parent = oldPath.substring(0, oldPath.lastIndexOf('/')) || '/';
    const toPath = parent === '/' ? `/${newName.value.trim()}` : `${parent}/${newName.value.trim()}`;

    await renameEntryApi(activeP.connectionId, oldPath, toPath);
    uiStore.showToast(`Renamed to ${newName.value}`, 'success');
    uiStore.isRenameOpen = false;

    // Immediately reload entries!
    await workspaceStore.refreshAll();
  } catch (err: any) {
    uiStore.showToast(err.response?.data?.error?.message || 'Rename failed', 'error');
  } finally {
    loading.value = false;
  }
}
</script>

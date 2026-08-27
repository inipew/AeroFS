<template>
  <div
    v-if="uiStore.isDeleteOpen"
    class="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4 select-none font-sans text-xs animate-in fade-in duration-150"
    @click="uiStore.isDeleteOpen = false"
  >
    <div
      class="bg-white dark:bg-[#0b0f19] border border-gray-200 dark:border-slate-800 rounded-3xl max-w-md w-full p-6 shadow-2xl space-y-4"
      @click.stop
    >
      <!-- Header -->
      <div class="flex items-center space-x-3">
        <div
          :class="[
            'w-10 h-10 rounded-2xl flex items-center justify-center shrink-0',
            deleteMode === 'permanent'
              ? 'bg-red-500/10 text-red-600 dark:text-red-400'
              : 'bg-amber-500/10 text-amber-600 dark:text-amber-400'
          ]"
        >
          <FbIcon name="trash" size="20px" />
        </div>
        <div>
          <h3 class="text-sm font-bold text-gray-900 dark:text-white">Delete Confirmation</h3>
          <p class="text-gray-500 dark:text-slate-400 text-xs">
            Choose whether to move to trash or delete permanently.
          </p>
        </div>
      </div>

      <!-- Delete Targets List -->
      <div class="space-y-1.5">
        <p class="text-gray-700 dark:text-slate-300 font-semibold text-xs">
          Target Items ({{ uiStore.deleteTargets.length }}):
        </p>
        <div class="max-h-28 overflow-y-auto bg-gray-50 dark:bg-slate-950 p-3 rounded-2xl border border-gray-200 dark:border-slate-800 font-mono text-[11px] text-gray-600 dark:text-slate-400 space-y-1 shadow-inner">
          <div v-for="p in uiStore.deleteTargets" :key="p" class="truncate flex items-center space-x-1.5">
            <span class="text-gray-400">•</span>
            <span class="truncate">{{ p }}</span>
          </div>
        </div>
      </div>

      <!-- Deletion Mode Choices (Recycle Bin vs Permanent) -->
      <div class="space-y-2 pt-1">
        <label class="block text-gray-700 dark:text-slate-300 font-semibold text-[11px]">Deletion Method:</label>
        <div class="grid grid-cols-1 gap-2">
          <!-- Option A: Move to Recycle Bin (Soft Delete) -->
          <label
            :class="[
              'p-3 rounded-2xl border flex items-start space-x-3 cursor-pointer transition select-none',
              deleteMode === 'trash'
                ? 'bg-blue-50 dark:bg-blue-900/30 border-blue-500/60 ring-2 ring-blue-500/20'
                : 'bg-white dark:bg-slate-900/60 border-gray-200 dark:border-slate-800 hover:border-gray-300 dark:hover:border-slate-700'
            ]"
          >
            <input
              type="radio"
              value="trash"
              v-model="deleteMode"
              class="mt-0.5 text-blue-600 focus:ring-0 cursor-pointer"
            />
            <div class="space-y-0.5">
              <div class="flex items-center space-x-1.5">
                <span class="font-bold text-gray-900 dark:text-white text-xs">Move to Recycle Bin</span>
                <span class="text-[10px] bg-emerald-100 dark:bg-emerald-950 text-emerald-700 dark:text-emerald-300 font-semibold px-1.5 py-0.2 rounded">Recommended</span>
              </div>
              <p class="text-[11px] text-gray-500 dark:text-slate-400">
                Items can be restored anytime from the Trash menu.
              </p>
            </div>
          </label>

          <!-- Option B: Delete Permanently -->
          <label
            :class="[
              'p-3 rounded-2xl border flex items-start space-x-3 cursor-pointer transition select-none',
              deleteMode === 'permanent'
                ? 'bg-red-50 dark:bg-red-900/30 border-red-500/60 ring-2 ring-red-500/20'
                : 'bg-white dark:bg-slate-900/60 border-gray-200 dark:border-slate-800 hover:border-gray-300 dark:hover:border-slate-700'
            ]"
          >
            <input
              type="radio"
              value="permanent"
              v-model="deleteMode"
              class="mt-0.5 text-red-600 focus:ring-0 cursor-pointer"
            />
            <div class="space-y-0.5">
              <span class="font-bold text-red-600 dark:text-red-400 text-xs">Delete Permanently</span>
              <p class="text-[11px] text-gray-500 dark:text-slate-400">
                Permanently purges files from disk. Cannot be undone.
              </p>
            </div>
          </label>
        </div>
      </div>

      <!-- Action Buttons -->
      <div class="flex justify-end space-x-2 pt-2 border-t border-gray-100 dark:border-slate-800">
        <button
          type="button"
          @click="uiStore.isDeleteOpen = false"
          class="px-4 py-2.5 rounded-xl text-gray-600 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800 transition font-medium text-xs cursor-pointer"
        >
          Cancel
        </button>

        <button
          type="button"
          :disabled="loading"
          @click="handleConfirmDelete"
          :class="[
            'px-5 py-2.5 rounded-xl font-semibold shadow-xs transition disabled:opacity-50 text-xs cursor-pointer flex items-center space-x-1.5 text-white',
            deleteMode === 'permanent'
              ? 'bg-red-600 hover:bg-red-700 active:bg-red-800'
              : 'bg-blue-600 hover:bg-blue-700 active:bg-blue-800'
          ]"
        >
          <span v-if="loading" class="animate-spin rounded-full h-3.5 w-3.5 border-2 border-white border-t-transparent"></span>
          <span>{{ loading ? 'Processing...' : (deleteMode === 'permanent' ? 'Delete Permanently' : 'Move to Trash') }}</span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { deleteFilesApi } from '../../api/files';
import { apiClient } from '../../api/client';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useFileStore } from '../../stores/fileStore';
import { useUiStore } from '../../stores/uiStore';

const workspaceStore = useWorkspaceStore();
const fileStore = useFileStore();
const uiStore = useUiStore();

const deleteMode = ref<'trash' | 'permanent'>('trash');
const loading = ref(false);

watch(
  () => uiStore.isDeleteOpen,
  (open) => {
    if (open) {
      deleteMode.value = 'trash'; // Default to safe recycle bin deletion
    }
  }
);

async function handleConfirmDelete() {
  if (uiStore.deleteTargets.length === 0) return;
  loading.value = true;

  try {
    const activeP = workspaceStore.getPanel(workspaceStore.activePanelId);
    const connId = activeP.connectionId || fileStore.currentConnectionId;

    if (deleteMode.value === 'trash') {
      // Soft delete: move to .trash
      const resp = await apiClient.post('/trash/move', {
        connection_id: connId,
        paths: uiStore.deleteTargets,
      });
      uiStore.showToast(resp.data.message || 'Moved item(s) to Recycle Bin', 'success');
    } else {
      // Permanent delete
      await deleteFilesApi(connId, uiStore.deleteTargets);
      uiStore.showToast(`Permanently deleted ${uiStore.deleteTargets.length} item(s)`, 'success');
    }

    uiStore.isDeleteOpen = false;

    // Immediately refresh workspace
    await workspaceStore.refreshAll();
  } catch (err: any) {
    uiStore.showToast(err.response?.data?.error?.message || 'Delete failed', 'error');
  } finally {
    loading.value = false;
  }
}
</script>

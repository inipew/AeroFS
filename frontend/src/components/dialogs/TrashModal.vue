<template>
  <div
    v-if="isOpen"
    class="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-3 sm:p-6 select-none font-sans text-xs animate-in fade-in duration-150"
    @click="isOpen = false"
  >
    <div
      class="bg-white dark:bg-[#0b0f19] border border-gray-200 dark:border-slate-800 rounded-3xl max-w-2xl w-full flex flex-col shadow-2xl overflow-hidden max-h-[85vh]"
      @click.stop
    >
      <!-- Header -->
      <div class="h-14 bg-gray-50 dark:bg-[#090d16] border-b border-gray-200 dark:border-slate-800 px-6 flex items-center justify-between text-xs shrink-0">
        <div class="flex items-center space-x-3">
          <div class="w-9 h-9 rounded-xl bg-red-500/10 text-red-600 dark:text-red-400 flex items-center justify-center shrink-0">
            <FbIcon name="trash" size="18px" />
          </div>
          <div>
            <h3 class="text-sm font-bold text-gray-900 dark:text-white">Trash & Recycle Bin</h3>
            <p class="text-[11px] text-gray-500 dark:text-slate-400">Restore accidentally deleted items or permanently purge them.</p>
          </div>
        </div>

        <!-- Actions: Empty Trash & Close -->
        <div class="flex items-center space-x-2">
          <button
            v-if="trashItems.length > 0"
            @click="handleEmptyTrash"
            class="px-3 py-1.5 bg-red-600 hover:bg-red-700 text-white font-semibold rounded-xl text-xs transition cursor-pointer shadow-xs"
          >
            Empty Trash
          </button>
          <button
            @click="isOpen = false"
            class="p-1.5 rounded-xl text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
          >
            ✕
          </button>
        </div>
      </div>

      <!-- Content -->
      <div class="p-6 overflow-y-auto flex-1 space-y-4">
        <!-- Empty State -->
        <div v-if="trashItems.length === 0 && !loading" class="py-16 text-center text-gray-400 dark:text-slate-500 space-y-2">
          <div class="w-12 h-12 rounded-2xl bg-gray-100 dark:bg-slate-900 mx-auto flex items-center justify-center text-gray-400">
            <FbIcon name="trash" size="24px" />
          </div>
          <p class="font-bold text-gray-700 dark:text-slate-300">Trash is empty</p>
          <p class="text-[11px]">Deleted items will appear here before permanent removal.</p>
        </div>

        <!-- Trash List -->
        <div v-else class="space-y-2.5">
          <div
            v-for="item in trashItems"
            :key="item.id"
            class="p-4 bg-gray-50 dark:bg-slate-900/60 rounded-2xl border border-gray-200 dark:border-slate-800 flex items-center justify-between gap-4"
          >
            <div class="truncate space-y-1">
              <div class="flex items-center space-x-2">
                <span class="font-bold text-gray-900 dark:text-white text-xs truncate">{{ item.item_name }}</span>
                <span class="text-[11px] text-gray-400 dark:text-slate-500">· Deleted {{ formatRelativeTime(item.deleted_at) }}</span>
              </div>
              <p class="text-[11px] text-gray-400 dark:text-slate-500 font-mono truncate">Original: {{ item.original_path }}</p>
            </div>

            <!-- Actions: Restore / Delete Permanently -->
            <div class="flex items-center space-x-2 shrink-0">
              <button
                @click="handleRestore(item.id)"
                class="px-3 py-1.5 bg-blue-50 hover:bg-blue-100 dark:bg-blue-950/40 dark:hover:bg-blue-900/50 text-blue-600 dark:text-blue-400 font-semibold rounded-xl text-xs transition cursor-pointer flex items-center space-x-1"
              >
                <span>↺ Restore</span>
              </button>
              <button
                @click="handlePermanentDelete(item.id)"
                class="px-3 py-1.5 bg-red-50 hover:bg-red-100 dark:bg-red-950/40 dark:hover:bg-red-900/50 text-red-600 dark:text-red-400 font-semibold rounded-xl text-xs transition cursor-pointer"
              >
                Delete
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { apiClient } from '../../api/client';
import { useUiStore } from '../../stores/uiStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';

const props = defineProps<{
  modelValue: boolean;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
}>();

const uiStore = useUiStore();
const workspaceStore = useWorkspaceStore();

const isOpen = ref(props.modelValue);
const trashItems = ref<any[]>([]);
const loading = ref(false);

watch(
  () => props.modelValue,
  (val) => {
    isOpen.value = val;
    if (val) {
      fetchTrash();
    }
  }
);

watch(
  () => isOpen.value,
  (val) => {
    emit('update:modelValue', val);
  }
);

async function fetchTrash() {
  loading.value = true;
  try {
    const resp = await apiClient.get('/trash');
    trashItems.value = resp.data;
  } catch (err: any) {
    uiStore.showToast(err.response?.data?.error?.message || 'Failed to load trash', 'error');
  } finally {
    loading.value = false;
  }
}

async function handleRestore(id: string) {
  try {
    await apiClient.post(`/trash/restore/${id}`);
    uiStore.showToast('Item restored successfully', 'success');
    await fetchTrash();
    await workspaceStore.refreshAll();
  } catch (err: any) {
    uiStore.showToast('Failed to restore item', 'error');
  }
}

async function handlePermanentDelete(id: string) {
  try {
    await apiClient.delete(`/trash/${id}`);
    uiStore.showToast('Item permanently deleted', 'info');
    await fetchTrash();
  } catch (err: any) {
    uiStore.showToast('Failed to delete item', 'error');
  }
}

async function handleEmptyTrash() {
  if (confirm('Are you sure you want to permanently empty the entire trash?')) {
    try {
      await apiClient.delete('/trash/empty');
      uiStore.showToast('Trash emptied', 'success');
      await fetchTrash();
    } catch (err: any) {
      uiStore.showToast('Failed to empty trash', 'error');
    }
  }
}

function formatRelativeTime(dateStr: string): string {
  const d = new Date(dateStr);
  return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
}
</script>

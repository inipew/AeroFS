<template>
  <div
    v-if="isOpen"
    class="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4 select-none font-sans text-xs animate-in fade-in duration-150"
  >
    <div class="bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-800 rounded-3xl max-w-sm w-full p-6 shadow-2xl">
      <div class="flex items-center space-x-3 mb-3">
        <div class="w-10 h-10 rounded-2xl bg-blue-600/10 dark:bg-blue-500/20 text-blue-600 dark:text-blue-400 flex items-center justify-center">
          <FbIcon name="archive" size="20px" />
        </div>
        <div>
          <h3 class="text-sm font-bold text-gray-900 dark:text-white">Create Archive</h3>
          <p class="text-gray-500 dark:text-slate-400 text-xs">
            Compress {{ selectedPaths.length }} selected item(s).
          </p>
        </div>
      </div>

      <form @submit.prevent="handleCompress" class="space-y-4">
        <div>
          <label class="block text-gray-700 dark:text-slate-300 text-[11px] font-semibold mb-1">Archive Name</label>
          <input
            v-model="archiveName"
            type="text"
            placeholder="archive.zip"
            class="w-full bg-gray-50 dark:bg-slate-950 border border-gray-200 dark:border-slate-800 rounded-xl px-3.5 py-2 text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:border-blue-500 text-xs shadow-inner"
            required
          />
        </div>

        <div>
          <label class="block text-gray-700 dark:text-slate-300 text-[11px] font-semibold mb-1">Format</label>
          <select
            v-model="format"
            class="w-full bg-gray-50 dark:bg-slate-950 border border-gray-200 dark:border-slate-800 rounded-xl px-3.5 py-2 text-gray-900 dark:text-white focus:outline-none focus:border-blue-500 text-xs shadow-inner cursor-pointer"
          >
            <option value="zip">ZIP (.zip) - Universal Archive</option>
            <option value="tar.gz">TAR.GZ (.tar.gz) - Gzip Tarball</option>
          </select>
        </div>

        <div class="flex justify-end space-x-2 pt-2 border-t border-gray-100 dark:border-slate-800">
          <button
            type="button"
            @click="isOpen = false"
            class="px-4 py-2 rounded-xl text-gray-600 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800 transition font-medium text-xs cursor-pointer"
          >
            Cancel
          </button>
          <button
            type="submit"
            :disabled="compressing || !archiveName.trim()"
            class="px-5 py-2 rounded-xl bg-blue-600 hover:bg-blue-700 text-white font-semibold shadow-xs transition disabled:opacity-50 text-xs cursor-pointer flex items-center space-x-1.5"
          >
            <span v-if="compressing" class="animate-spin rounded-full h-3 w-3 border-2 border-white border-t-transparent"></span>
            <span>{{ compressing ? 'Compressing...' : 'Create Archive' }}</span>
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
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useUiStore } from '../../stores/uiStore';

const props = defineProps<{
  modelValue: boolean;
  connectionId: string;
  basePath: string;
  selectedPaths: string[];
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
}>();

const workspaceStore = useWorkspaceStore();
const uiStore = useUiStore();

const isOpen = ref(props.modelValue);
const archiveName = ref('archive.zip');
const format = ref<'zip' | 'tar.gz'>('zip');
const compressing = ref(false);

watch(
  () => props.modelValue,
  (val) => {
    isOpen.value = val;
    if (val) {
      archiveName.value = `archive_${Date.now().toString().slice(-4)}.zip`;
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
  () => format.value,
  (f) => {
    const base = archiveName.value.replace(/\.(zip|tar\.gz|tgz)$/i, '');
    archiveName.value = `${base}.${f}`;
  }
);

async function handleCompress() {
  if (!archiveName.value.trim()) return;
  compressing.value = true;

  try {
    const destFile = props.basePath === '/'
      ? `/${archiveName.value.trim()}`
      : `${props.basePath}/${archiveName.value.trim()}`;

    await apiClient.post(`/connections/${props.connectionId}/archive/compress`, {
      base_path: props.basePath,
      relative_paths: props.selectedPaths,
      destination_file: destFile,
      format: format.value,
    });

    uiStore.showToast(`Created archive: ${archiveName.value}`, 'success');
    isOpen.value = false;
    await workspaceStore.refreshAll();
  } catch (err: any) {
    uiStore.showToast(err.response?.data?.error?.message || 'Compression failed', 'error');
  } finally {
    compressing.value = false;
  }
}
</script>

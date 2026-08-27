<template>
  <div
    v-if="isOpen"
    class="fixed inset-0 z-50 bg-black/75 backdrop-blur-sm flex items-start justify-center pt-20 p-4 select-none font-sans text-xs"
    @click.self="isOpen = false"
  >
    <div class="bg-slate-900 border border-slate-700 rounded-2xl max-w-xl w-full shadow-2xl overflow-hidden flex flex-col max-h-[75vh] animate-in zoom-in-95 duration-100">
      <!-- Search Input Bar -->
      <div class="p-3.5 bg-slate-950 border-b border-slate-800 flex items-center space-x-3">
        <span class="text-slate-400 text-sm">🔍</span>
        <input
          ref="inputRef"
          v-model="query"
          type="text"
          placeholder="Search files and folders recursively (e.g. *.ts, report)..."
          class="flex-1 bg-transparent text-white placeholder-slate-500 focus:outline-none text-xs"
          @keydown.esc="isOpen = false"
        />

        <!-- Regex Toggle -->
        <button
          @click="isRegex = !isRegex"
          :class="[
            'px-2 py-1 rounded text-[10px] font-mono transition border',
            isRegex ? 'bg-indigo-600/30 border-indigo-500 text-indigo-300' : 'bg-slate-900 border-slate-700 text-slate-500'
          ]"
          title="Toggle Regular Expression"
        >
          .*
        </button>

        <span class="text-[10px] text-slate-500 font-mono">ESC to close</span>
      </div>

      <!-- Results List -->
      <div class="flex-1 overflow-y-auto p-2 space-y-1 divide-y divide-slate-800/40">
        <div v-if="loading" class="py-8 text-center text-slate-500">
          <div class="inline-block animate-spin rounded-full h-4 w-4 border-2 border-indigo-500 border-t-transparent mb-1"></div>
          <p>Searching...</p>
        </div>

        <div v-else-if="results.length === 0 && query.trim()" class="py-8 text-center text-slate-500">
          No files found matching "<span class="text-slate-300">{{ query }}</span>"
        </div>

        <div v-else-if="!query.trim()" class="py-8 text-center text-slate-600">
          Type to search files across current connection
        </div>

        <div
          v-for="entry in results"
          :key="entry.path"
          @click="handleSelect(entry)"
          class="p-2 rounded-lg hover:bg-slate-800 cursor-pointer flex items-center justify-between transition group text-xs"
        >
          <div class="flex items-center space-x-2.5 truncate max-w-[420px]">
            <span class="text-sm">
              {{ entry.kind === 'directory' ? '📁' : (entry.kind === 'symlink' ? '🔗' : '📄') }}
            </span>
            <div class="truncate">
              <p class="font-medium text-slate-200 group-hover:text-white truncate">
                {{ entry.name }}
              </p>
              <p class="text-[10px] text-slate-500 font-mono truncate">
                {{ entry.path }}
              </p>
            </div>
          </div>

          <div class="text-right text-[10px] text-slate-500 font-mono">
            {{ entry.kind === 'directory' ? 'Folder' : formatBytes(entry.size || 0) }}
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, nextTick } from 'vue';
import { apiClient } from '../../api/client';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import type { FileEntry } from '../../types/vfs';

const props = defineProps<{
  modelValue: boolean;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
}>();

const workspaceStore = useWorkspaceStore();

const isOpen = ref(props.modelValue);
const query = ref('');
const isRegex = ref(false);
const loading = ref(false);
const results = ref<FileEntry[]>([]);
const inputRef = ref<HTMLInputElement | null>(null);

let debounceTimer: any = null;

watch(
  () => props.modelValue,
  (val) => {
    isOpen.value = val;
    if (val) {
      nextTick(() => inputRef.value?.focus());
    }
  }
);

watch(
  () => isOpen.value,
  (val) => {
    emit('update:modelValue', val);
  }
);

watch([query, isRegex], () => {
  if (debounceTimer) clearTimeout(debounceTimer);
  if (!query.value.trim()) {
    results.value = [];
    return;
  }

  debounceTimer = setTimeout(() => {
    performSearch();
  }, 250);
});

async function performSearch() {
  if (!query.value.trim()) return;
  loading.value = true;

  try {
    const activeP = workspaceStore.getPanel(workspaceStore.activePanelId);
    const resp = await apiClient.get<FileEntry[]>(`/connections/${activeP.connectionId}/search`, {
      params: {
        path: '/',
        query: query.value.trim(),
        regex: isRegex.value,
      },
    });
    results.value = resp.data;
  } catch {
    results.value = [];
  } finally {
    loading.value = false;
  }
}

function handleSelect(entry: FileEntry) {
  const activeP = workspaceStore.getPanel(workspaceStore.activePanelId);
  const parentPath = entry.path.substring(0, entry.path.lastIndexOf('/')) || '/';
  
  workspaceStore.navigatePanel(workspaceStore.activePanelId, parentPath);
  activeP.selectedEntries = [entry.path];
  isOpen.value = false;
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}
</script>

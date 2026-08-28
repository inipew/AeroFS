<template>
  <Transition name="spotlight-modal">
    <div
      v-if="isOpen"
      class="fixed inset-0 z-50 bg-black/60 backdrop-blur-xs flex items-start justify-center pt-20 p-4 select-none font-sans text-xs"
      @click.self="isOpen = false"
    >
      <div class="spotlight-card bg-white dark:bg-[#0f1422] border border-gray-200 dark:border-slate-800 rounded-3xl max-w-xl w-full shadow-2xl overflow-hidden flex flex-col max-h-[80vh]">
      <!-- Search Input Bar -->
      <div class="p-3.5 bg-gray-50/80 dark:bg-slate-950/80 border-b border-gray-200 dark:border-slate-800/80 flex items-center space-x-3">
        <FbIcon name="search" size="18px" class="text-gray-400 dark:text-slate-500 shrink-0" />
        <input
          ref="inputRef"
          v-model="query"
          type="text"
          placeholder="Search files and folders (e.g. *.ts, report)..."
          class="flex-1 bg-transparent text-gray-900 dark:text-white placeholder-gray-400 dark:placeholder-slate-500 focus:outline-none text-xs"
          @keydown.esc="isOpen = false"
        />

        <!-- Regex Toggle -->
        <button
          @click="isRegex = !isRegex"
          :class="[
            'px-2 py-1 rounded-lg text-[10px] font-mono transition border cursor-pointer font-bold',
            isRegex
              ? 'bg-blue-500/20 border-blue-500 text-blue-600 dark:text-blue-400'
              : 'bg-gray-100 dark:bg-slate-800 border-gray-200 dark:border-slate-700 text-gray-500 dark:text-slate-400 hover:text-gray-900 dark:hover:text-white'
          ]"
          title="Toggle Regular Expression"
        >
          .*
        </button>

        <span class="text-[10px] text-gray-400 dark:text-slate-500 font-mono px-1.5 py-0.5 rounded bg-gray-100 dark:bg-slate-800">
          ESC
        </span>
      </div>

      <!-- Scope Filter Tabs & Type Chips -->
      <div class="px-4 py-2 border-b border-gray-100 dark:border-slate-800/80 bg-white dark:bg-[#0f1422] flex flex-wrap items-center justify-between gap-2 text-[11px]">
        <div class="flex items-center space-x-1.5 overflow-x-auto">
          <span class="text-gray-400 dark:text-slate-500 text-[10px] font-bold uppercase tracking-wider mr-1">Scope:</span>
          <button
            v-for="sc in scopeOptions"
            :key="sc.id"
            @click="scope = sc.id"
            :class="[
              'px-2.5 py-0.5 rounded-xl transition cursor-pointer font-medium text-xs',
              scope === sc.id
                ? 'bg-blue-600 text-white font-semibold shadow-xs'
                : 'text-gray-500 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800 hover:text-gray-900 dark:hover:text-white'
            ]"
          >
            {{ sc.label }}
          </button>
        </div>

        <div class="flex items-center space-x-1 overflow-x-auto">
          <button
            v-for="cat in ['all', 'code', 'images', 'archives', 'documents', 'large']"
            :key="cat"
            @click="filterType = cat as any"
            :class="[
              'px-2 py-0.5 rounded-lg transition cursor-pointer text-[10px] font-medium capitalize',
              filterType === cat
                ? 'bg-blue-500/20 text-blue-600 dark:text-blue-400 font-bold border border-blue-500/30'
                : 'text-gray-400 dark:text-slate-500 hover:bg-gray-100 dark:hover:bg-slate-800'
            ]"
          >
            {{ cat === 'large' ? '> 10 MB' : cat }}
          </button>
        </div>
      </div>

      <!-- Results List -->
      <div class="flex-1 overflow-y-auto p-2 space-y-1">
        <div v-if="loading" class="py-12 text-center text-gray-400 dark:text-slate-500">
          <div class="inline-block animate-spin rounded-full h-5 w-5 border-2 border-blue-600 border-t-transparent mb-2"></div>
          <p class="font-medium">Searching files...</p>
        </div>

        <div v-else-if="filteredResults.length === 0 && query.trim()" class="py-12 text-center text-gray-400 dark:text-slate-500">
          No files found matching "<span class="text-gray-700 dark:text-slate-200 font-semibold">{{ query }}</span>"
        </div>

        <div v-else-if="!query.trim()" class="py-12 text-center text-gray-400 dark:text-slate-500">
          <FbIcon name="search" size="24px" class="mx-auto mb-2 opacity-50" />
          <p class="font-medium">Type a keyword to start searching</p>
        </div>

        <div
          v-for="res in filteredResults"
          :key="`${res.connectionId}:${res.entry.path}`"
          @click="handleSelect(res)"
          class="p-2.5 rounded-2xl hover:bg-gray-100 dark:hover:bg-slate-800/80 cursor-pointer flex items-center justify-between transition group text-xs border border-transparent hover:border-gray-200 dark:hover:border-slate-700"
        >
          <div class="flex items-center space-x-2.5 truncate max-w-[420px]">
            <FbIcon
              :name="getEntryIcon(res.entry)"
              size="18px"
              :class="res.entry.kind === 'directory' ? 'text-blue-600 dark:text-blue-400' : 'text-gray-400 dark:text-slate-500'"
            />
            <div class="truncate">
              <p class="font-semibold text-gray-900 dark:text-white group-hover:text-blue-600 dark:group-hover:text-blue-400 truncate">
                {{ res.entry.name }}
              </p>
              <div class="flex items-center space-x-1.5 text-[10px] text-gray-400 dark:text-slate-500 font-mono truncate">
                <span v-if="scope === 'all_conns'" class="px-1 py-0.2 rounded bg-blue-50 dark:bg-blue-950/40 text-blue-600 dark:text-blue-400 font-sans font-bold">
                  {{ getConnectionName(res.connectionId) }}
                </span>
                <span class="truncate">{{ res.entry.path }}</span>
              </div>
            </div>
          </div>

          <div class="text-right text-[10px] text-gray-400 dark:text-slate-500 font-mono shrink-0 ml-2">
            {{ res.entry.kind === 'directory' ? 'Folder' : formatBytes(res.entry.size || 0) }}
          </div>
        </div>
      </div>
    </div>
  </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { apiClient } from '../../api/client';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useConnectionStore } from '../../stores/connectionStore';
import type { FileEntry } from '../../types/vfs';

import type { IconName } from '../../utils/icons';

interface SearchResultItem {
  connectionId: string;
  entry: FileEntry;
}

const props = defineProps<{
  modelValue: boolean;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
}>();

const workspaceStore = useWorkspaceStore();
const connStore = useConnectionStore();

const isOpen = ref(props.modelValue);
const query = ref('');
const isRegex = ref(false);
const scope = ref<'current_dir' | 'current_conn' | 'all_conns'>('current_dir');
const filterType = ref<'all' | 'code' | 'images' | 'archives' | 'documents' | 'large'>('all');
const loading = ref(false);
const results = ref<SearchResultItem[]>([]);
const inputRef = ref<HTMLInputElement | null>(null);

const scopeOptions = [
  { id: 'current_dir' as const, label: 'Current Folder' },
  { id: 'current_conn' as const, label: 'Current Connection' },
  { id: 'all_conns' as const, label: 'All Connections' },
];

const filteredResults = computed(() => {
  if (filterType.value === 'all') return results.value;
  return results.value.filter((r) => {
    if (r.entry.kind === 'directory') return true;
    const ext = r.entry.name.split('.').pop()?.toLowerCase() || '';
    if (filterType.value === 'code') {
      return ['rs', 'ts', 'js', 'vue', 'html', 'css', 'py', 'json', 'toml', 'yaml', 'sh', 'sql', 'c', 'cpp', 'go', 'java'].includes(ext);
    }
    if (filterType.value === 'images') {
      return ['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp', 'bmp', 'avif'].includes(ext);
    }
    if (filterType.value === 'archives') {
      return ['zip', 'tar', 'gz', 'tgz', '7z', 'rar', 'bz2', 'xz'].includes(ext);
    }
    if (filterType.value === 'documents') {
      return ['pdf', 'doc', 'docx', 'txt', 'md', 'csv', 'xlsx'].includes(ext);
    }
    if (filterType.value === 'large') {
      return (r.entry.size || 0) > 10 * 1024 * 1024; // > 10 MB
    }
    return true;
  });
});

function getEntryIcon(entry: FileEntry): IconName {
  if (entry.kind === 'directory') return 'folder';
  const ext = entry.name.split('.').pop()?.toLowerCase() || '';
  if (['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp'].includes(ext)) return 'image';
  if (['mp4', 'webm', 'mov', 'mkv'].includes(ext)) return 'video';
  if (['mp3', 'wav', 'ogg', 'flac'].includes(ext)) return 'audio';
  if (['zip', 'tar', 'gz', 'tgz', '7z', 'rar'].includes(ext)) return 'archive';
  if (['pdf'].includes(ext)) return 'pdf';
  return 'file';
}

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

watch([query, isRegex, scope], () => {
  if (debounceTimer) clearTimeout(debounceTimer);
  if (!query.value.trim()) {
    results.value = [];
    return;
  }

  debounceTimer = setTimeout(() => {
    performSearch();
  }, 250);
});

function getConnectionName(connId: string): string {
  const conn = connStore.connections.find((c) => c.id === connId);
  return conn ? conn.name : connId;
}

async function performSearch() {
  if (!query.value.trim()) return;
  loading.value = true;
  results.value = [];

  try {
    const activeP = workspaceStore.getPanel(workspaceStore.activePanelId);

    if (scope.value === 'all_conns') {
      const allResults: SearchResultItem[] = [];
      for (const conn of connStore.connections) {
        try {
          const resp = await apiClient.get<any>(`/connections/${conn.id}/search`, {
            params: {
              path: '/',
              query: query.value.trim(),
              regex: isRegex.value,
            },
          });
          const items: FileEntry[] = Array.isArray(resp.data) ? resp.data : resp.data.results || [];
          for (const item of items) {
            allResults.push({ connectionId: conn.id, entry: item });
          }
        } catch {
          // Continue searching other connections
        }
      }
      results.value = allResults;
    } else {
      const searchPath = scope.value === 'current_dir' ? activeP.location.path : '/';
      const resp = await apiClient.get<any>(`/connections/${activeP.location.connectionId}/search`, {
        params: {
          path: searchPath,
          query: query.value.trim(),
          regex: isRegex.value,
        },
      });
      const items: FileEntry[] = Array.isArray(resp.data) ? resp.data : resp.data.results || [];
      results.value = items.map((item) => ({
        connectionId: activeP.location.connectionId,
        entry: item,
      }));
    }
  } catch {
    results.value = [];
  } finally {
    loading.value = false;
  }
}

async function handleSelect(res: SearchResultItem) {
  const activeP = workspaceStore.getPanel(workspaceStore.activePanelId);
  const parentPath = res.entry.path.substring(0, res.entry.path.lastIndexOf('/')) || '/';

  if (activeP.location.connectionId !== res.connectionId) {
    await workspaceStore.switchPanelConnection(workspaceStore.activePanelId, res.connectionId, parentPath);
  } else {
    await workspaceStore.navigateTo(workspaceStore.activePanelId, parentPath);
  }

  activeP.selectedEntries = [res.entry.path];
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

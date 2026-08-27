<template>
  <div class="flex-1 overflow-auto bg-slate-950 p-4 select-none text-xs text-slate-300">
    <!-- Empty State -->
    <div v-if="fileStore.filteredEntries.length === 0 && !fileStore.loading" class="py-16 text-center text-slate-600">
      <p class="text-base">Folder is empty</p>
      <p class="text-xs text-slate-700 mt-1">Upload or create a file to get started</p>
    </div>

    <!-- Grid Container -->
    <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6 xl:grid-cols-8 gap-3">
      <div
        v-for="entry in fileStore.filteredEntries"
        :key="entry.path"
        @click="handleClick($event, entry)"
        @dblclick="handleDoubleClick(entry)"
        @contextmenu="uiStore.openContextMenu($event, entry)"
        :class="[
          'flex flex-col items-center p-3 rounded-lg border cursor-pointer transition text-center group',
          fileStore.selectedEntries.includes(entry.path)
            ? 'bg-indigo-600/20 border-indigo-500/50 text-indigo-200'
            : 'bg-slate-900/40 border-slate-800/80 hover:bg-slate-900 hover:border-slate-700 text-slate-300'
        ]"
      >
        <!-- Icon -->
        <div class="text-3xl mb-2">
          <span v-if="entry.kind === 'directory'">📁</span>
          <span v-else-if="entry.kind === 'symlink'">🔗</span>
          <span v-else>📄</span>
        </div>

        <!-- Name -->
        <span class="w-full truncate font-medium text-xs group-hover:text-white" :title="entry.name">
          {{ entry.name }}
        </span>

        <!-- Subtitle (Size / Item count) -->
        <span class="text-[10px] text-slate-500 mt-1">
          {{ entry.kind === 'directory' ? 'Folder' : formatBytes(entry.size || 0) }}
        </span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useFileStore } from '../../stores/fileStore';
import { useUiStore } from '../../stores/uiStore';
import { getDownloadUrl } from '../../api/files';
import type { FileEntry } from '../../types/vfs';

const fileStore = useFileStore();
const uiStore = useUiStore();

function handleClick(e: MouseEvent, entry: FileEntry) {
  fileStore.toggleSelect(entry.path, e.ctrlKey || e.metaKey);
}

function handleDoubleClick(entry: FileEntry) {
  if (entry.kind === 'directory') {
    fileStore.navigateTo(entry.path);
  } else {
    const url = getDownloadUrl(fileStore.currentConnectionId, entry.path);
    window.open(url, '_blank');
  }
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}
</script>

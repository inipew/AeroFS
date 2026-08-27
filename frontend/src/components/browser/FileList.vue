<template>
  <div class="flex-1 overflow-auto bg-slate-950 select-none text-xs text-slate-300">
    <table class="w-full text-left border-collapse">
      <thead class="bg-slate-900/80 sticky top-0 border-b border-slate-800 text-[11px] text-slate-400 font-semibold uppercase tracking-wider">
        <tr>
          <th class="py-2.5 px-3 w-10 text-center">
            <input
              type="checkbox"
              :checked="isAllSelected"
              @change="toggleSelectAll"
              class="rounded bg-slate-900 border-slate-700 text-indigo-600 focus:ring-0 cursor-pointer"
            />
          </th>
          <th class="py-2.5 px-3 cursor-pointer hover:text-white transition" @click="setSort('name')">
            Name <span v-if="fileStore.sortField === 'name'">{{ fileStore.sortOrder === 'asc' ? '↑' : '↓' }}</span>
          </th>
          <th class="py-2.5 px-3 w-28 text-right cursor-pointer hover:text-white transition" @click="setSort('size')">
            Size <span v-if="fileStore.sortField === 'size'">{{ fileStore.sortOrder === 'asc' ? '↑' : '↓' }}</span>
          </th>
          <th class="py-2.5 px-3 w-40 cursor-pointer hover:text-white transition" @click="setSort('modified')">
            Modified <span v-if="fileStore.sortField === 'modified'">{{ fileStore.sortOrder === 'asc' ? '↑' : '↓' }}</span>
          </th>
          <th class="py-2.5 px-3 w-24 text-center">Perms</th>
        </tr>
      </thead>
      <tbody class="divide-y divide-slate-900">
        <!-- Empty State -->
        <tr v-if="fileStore.filteredEntries.length === 0 && !fileStore.loading">
          <td colspan="5" class="py-12 text-center text-slate-600">
            <p class="text-sm">Folder is empty</p>
            <p class="text-[11px] text-slate-700 mt-1">Upload or create a file to get started</p>
          </td>
        </tr>

        <!-- Loading State -->
        <tr v-if="fileStore.loading">
          <td colspan="5" class="py-12 text-center text-slate-500">
            <div class="inline-block animate-spin rounded-full h-5 w-5 border-2 border-indigo-500 border-t-transparent mb-2"></div>
            <p>Loading files...</p>
          </td>
        </tr>

        <!-- File / Dir Rows -->
        <tr
          v-for="entry in fileStore.filteredEntries"
          :key="entry.path"
          @click="handleClick($event, entry)"
          @dblclick="handleDoubleClick(entry)"
          @contextmenu="uiStore.openContextMenu($event, entry)"
          :class="[
            'cursor-pointer transition group',
            fileStore.selectedEntries.includes(entry.path)
              ? 'bg-indigo-600/15 text-indigo-200'
              : 'hover:bg-slate-900/60'
          ]"
        >
          <!-- Select Box -->
          <td class="py-2 px-3 text-center" @click.stop>
            <input
              type="checkbox"
              :checked="fileStore.selectedEntries.includes(entry.path)"
              @change="fileStore.toggleSelect(entry.path, true)"
              class="rounded bg-slate-900 border-slate-700 text-indigo-600 focus:ring-0 cursor-pointer"
            />
          </td>

          <!-- File / Folder Name & Icon -->
          <td class="py-2 px-3 flex items-center space-x-2.5 font-medium truncate">
            <span class="text-base leading-none">
              <span v-if="entry.kind === 'directory'">📁</span>
              <span v-else-if="entry.kind === 'symlink'">🔗</span>
              <span v-else>📄</span>
            </span>
            <span class="truncate" :class="{ 'text-indigo-300 font-semibold': entry.kind === 'directory' }">
              {{ entry.name }}
            </span>
          </td>

          <!-- Size -->
          <td class="py-2 px-3 text-right text-slate-400 font-mono text-[11px]">
            {{ entry.kind === 'directory' ? '-' : formatBytes(entry.size || 0) }}
          </td>

          <!-- Modified Date -->
          <td class="py-2 px-3 text-slate-500 text-[11px] truncate">
            {{ formatDate(entry.modified_at) }}
          </td>

          <!-- Permissions -->
          <td class="py-2 px-3 text-center text-slate-500 font-mono text-[10px]">
            {{ entry.permissions || '-' }}
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useFileStore } from '../../stores/fileStore';
import { useUiStore } from '../../stores/uiStore';
import { getDownloadUrl } from '../../api/files';
import type { FileEntry } from '../../types/vfs';

const fileStore = useFileStore();
const uiStore = useUiStore();

const isAllSelected = computed(() => {
  return (
    fileStore.filteredEntries.length > 0 &&
    fileStore.selectedEntries.length === fileStore.filteredEntries.length
  );
});

function toggleSelectAll() {
  if (isAllSelected.value) {
    fileStore.clearSelection();
  } else {
    fileStore.selectAll();
  }
}

function setSort(field: string) {
  if (fileStore.sortField === field) {
    fileStore.sortOrder = fileStore.sortOrder === 'asc' ? 'desc' : 'asc';
  } else {
    fileStore.sortField = field;
    fileStore.sortOrder = 'asc';
  }
  fileStore.fetchEntries();
}

function handleClick(e: MouseEvent, entry: FileEntry) {
  fileStore.toggleSelect(entry.path, e.ctrlKey || e.metaKey);
}

function handleDoubleClick(entry: FileEntry) {
  if (entry.kind === 'directory') {
    fileStore.navigateTo(entry.path);
  } else {
    // Trigger download
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

function formatDate(dateStr?: string): string {
  if (!dateStr) return '-';
  const d = new Date(dateStr);
  return d.toLocaleString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}
</script>

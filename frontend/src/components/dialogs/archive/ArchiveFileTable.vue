<template>
  <div class="flex-1 flex flex-col min-h-0 bg-white dark:bg-[#111317] overflow-hidden select-none">
    <!-- Table Header with Sortable Columns -->
    <div class="h-9 px-3 flex items-center border-b border-gray-200/80 dark:border-white/[0.08] bg-gray-50/70 dark:bg-[#0c0e12]/80 text-[11px] font-semibold text-gray-500 dark:text-slate-400 shrink-0 select-none">
      <!-- Checkbox Column -->
      <div class="w-8 flex items-center justify-center shrink-0">
        <input
          type="checkbox"
          :checked="isAllSelected"
          :indeterminate="isPartiallySelected"
          @change="$emit('toggleSelectAll')"
          class="rounded border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-blue-500/30 cursor-pointer accent-blue-600"
          title="Select all"
        />
      </div>

      <!-- Name Column -->
      <div
        @click="setSort('name')"
        class="flex-1 min-w-[180px] flex items-center space-x-1.5 cursor-pointer hover:text-gray-900 dark:hover:text-slate-200 transition px-2"
      >
        <span>Name</span>
        <FbIcon
          v-if="sortField === 'name'"
          :name="sortAsc ? 'arrow-up' : 'arrow-down'"
          size="11px"
          class="text-blue-500"
        />
      </div>

      <!-- Original Size -->
      <div
        @click="setSort('size')"
        class="w-24 text-right pr-3 hidden sm:flex items-center justify-end space-x-1 cursor-pointer hover:text-gray-900 dark:hover:text-slate-200 transition shrink-0"
      >
        <span>Original</span>
        <FbIcon
          v-if="sortField === 'size'"
          :name="sortAsc ? 'arrow-up' : 'arrow-down'"
          size="11px"
          class="text-blue-500"
        />
      </div>

      <!-- Compressed Size + Ratio -->
      <div
        @click="setSort('compressed_size')"
        class="w-32 text-right pr-3 hidden md:flex items-center justify-end space-x-1 cursor-pointer hover:text-gray-900 dark:hover:text-slate-200 transition shrink-0"
      >
        <span>Compressed</span>
        <FbIcon
          v-if="sortField === 'compressed_size'"
          :name="sortAsc ? 'arrow-up' : 'arrow-down'"
          size="11px"
          class="text-blue-500"
        />
      </div>

      <!-- Type Column -->
      <div
        @click="setSort('type')"
        class="w-24 px-2 hidden lg:flex items-center space-x-1 cursor-pointer hover:text-gray-900 dark:hover:text-slate-200 transition shrink-0"
      >
        <span>Type</span>
        <FbIcon
          v-if="sortField === 'type'"
          :name="sortAsc ? 'arrow-up' : 'arrow-down'"
          size="11px"
          class="text-blue-500"
        />
      </div>

      <!-- Modified Date -->
      <div
        @click="setSort('modified')"
        class="w-32 text-right pr-3 hidden xl:flex items-center justify-end space-x-1 cursor-pointer hover:text-gray-900 dark:hover:text-slate-200 transition shrink-0"
      >
        <span>Modified</span>
        <FbIcon
          v-if="sortField === 'modified'"
          :name="sortAsc ? 'arrow-up' : 'arrow-down'"
          size="11px"
          class="text-blue-500"
        />
      </div>
    </div>

    <!-- Table Body -->
    <div class="flex-1 overflow-y-auto divide-y divide-gray-100/80 dark:divide-white/[0.04] scrollbar-thin">
      <!-- Loading Skeleton -->
      <div v-if="isLoading" class="py-16 flex flex-col items-center justify-center space-y-2 text-gray-400">
        <div class="w-6 h-6 border-2 border-blue-500 border-t-transparent rounded-full animate-spin"></div>
        <span class="text-xs">Reading virtual archive index...</span>
      </div>

      <!-- Empty State -->
      <div
        v-else-if="sortedEntries.length === 0"
        class="py-20 flex flex-col items-center justify-center text-gray-400 dark:text-slate-500 space-y-2"
      >
        <FbIcon name="folder" size="32px" class="opacity-40" />
        <span class="text-xs font-medium">This folder inside the archive is empty</span>
      </div>

      <!-- Rows -->
      <div
        v-else
        v-for="entry in sortedEntries"
        :key="entry.path"
        @click="$emit('select', entry, $event)"
        @dblclick="handleDoubleClick(entry)"
        :class="[
          'h-9 sm:h-[38px] px-3 flex items-center text-xs transition-colors duration-100 cursor-pointer group',
          selectedPaths.includes(entry.path)
            ? 'bg-blue-500/10 dark:bg-white/[0.07] text-blue-600 dark:text-white font-medium'
            : (activeEntry?.path === entry.path
                ? 'bg-gray-100/70 dark:bg-white/[0.04] text-gray-900 dark:text-slate-100'
                : 'text-gray-700 dark:text-slate-300 hover:bg-gray-100/50 dark:hover:bg-white/[0.035]')
        ]"
      >
        <!-- Checkbox -->
        <div class="w-8 flex items-center justify-center shrink-0" @click.stop>
          <input
            type="checkbox"
            :checked="selectedPaths.includes(entry.path)"
            @change="$emit('toggleSelect', entry.path)"
            class="rounded border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-blue-500/30 cursor-pointer accent-blue-600"
          />
        </div>

        <!-- Name Column with Icon & Substring Highlight -->
        <div class="flex-1 min-w-[180px] flex items-center space-x-2.5 px-2 truncate">
          <FbIcon
            :name="getEntryIcon(entry)"
            size="15px"
            :class="getIconColorClass(entry)"
            class="shrink-0"
          />
          <span class="truncate font-medium" :title="entry.name">
            <template v-if="searchQuery">
              <span v-html="highlightMatch(entry.name, searchQuery)"></span>
            </template>
            <template v-else>
              {{ entry.name }}
            </template>
          </span>
          <span
            v-if="isArchiveFile(entry.name)"
            class="px-1.5 py-0.2 rounded text-[9px] font-mono font-bold uppercase bg-amber-500/10 text-amber-600 dark:text-amber-400 border border-amber-500/20 shrink-0"
          >
            Archive
          </span>
        </div>

        <!-- Original Size -->
        <div class="w-24 text-right pr-3 font-mono text-[11px] text-gray-500 dark:text-slate-400 hidden sm:block shrink-0">
          {{ entry.kind === 'directory' ? '—' : formatBytes(entry.size || 0) }}
        </div>

        <!-- Compressed Size & % Saved Badge -->
        <div class="w-32 text-right pr-3 font-mono text-[11px] hidden md:flex items-center justify-end space-x-1.5 shrink-0">
          <template v-if="entry.kind === 'directory'">
            <span class="text-gray-400 dark:text-slate-600">—</span>
          </template>
          <template v-else>
            <span class="text-gray-500 dark:text-slate-400">
              {{ entry.compressed_size ? formatBytes(entry.compressed_size) : formatBytes(entry.size || 0) }}
            </span>
            <span
              v-if="calcSavedPercent(entry.size, entry.compressed_size) > 0"
              class="px-1 py-0.2 rounded text-[9px] font-bold bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border border-emerald-500/20"
              title="Compression savings"
            >
              {{ calcSavedPercent(entry.size, entry.compressed_size) }}%
            </span>
          </template>
        </div>

        <!-- Type Badge -->
        <div class="w-24 px-2 hidden lg:flex items-center shrink-0">
          <span
            class="px-1.5 py-0.5 rounded-md text-[10px] font-medium tracking-wide truncate max-w-[85px]"
            :class="getTypeBadgeClass(entry)"
          >
            {{ getTypeLabel(entry) }}
          </span>
        </div>

        <!-- Modified Date -->
        <div class="w-32 text-right pr-3 font-mono text-[11px] text-gray-400 dark:text-slate-500 hidden xl:block shrink-0 truncate">
          {{ formatModifiedDate(entry.modified_at) }}
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import FbIcon from '../../common/FbIcon.vue';
import type { IconName } from '../../../utils/icons';
import type { VirtualArchiveEntry } from '../../../api/archive';

const props = defineProps<{
  entries: VirtualArchiveEntry[];
  selectedPaths: string[];
  activeEntry: VirtualArchiveEntry | null;
  searchQuery: string;
  isLoading: boolean;
}>();

const emit = defineEmits<{
  (e: 'select', entry: VirtualArchiveEntry, event: MouseEvent): void;
  (e: 'toggleSelect', path: string): void;
  (e: 'toggleSelectAll'): void;
  (e: 'navigate', path: string): void;
  (e: 'openPreview', entry: VirtualArchiveEntry): void;
}>();

type SortField = 'name' | 'size' | 'compressed_size' | 'type' | 'modified';

const sortField = ref<SortField>('name');
const sortAsc = ref(true);

function setSort(field: SortField) {
  if (sortField.value === field) {
    sortAsc.value = !sortAsc.value;
  } else {
    sortField.value = field;
    sortAsc.value = true;
  }
}

const isAllSelected = computed(() => {
  return (
    props.entries.length > 0 &&
    props.entries.every((e) => props.selectedPaths.includes(e.path))
  );
});

const isPartiallySelected = computed(() => {
  const count = props.entries.filter((e) => props.selectedPaths.includes(e.path)).length;
  return count > 0 && count < props.entries.length;
});

const sortedEntries = computed(() => {
  const list = [...props.entries];
  list.sort((a, b) => {
    // Directories always first
    if (a.kind === 'directory' && b.kind !== 'directory') return -1;
    if (a.kind !== 'directory' && b.kind === 'directory') return 1;

    let res = 0;
    if (sortField.value === 'name') {
      res = a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' });
    } else if (sortField.value === 'size') {
      res = (a.size || 0) - (b.size || 0);
    } else if (sortField.value === 'compressed_size') {
      res = (a.compressed_size || a.size || 0) - (b.compressed_size || b.size || 0);
    } else if (sortField.value === 'type') {
      res = getTypeLabel(a).localeCompare(getTypeLabel(b));
    } else if (sortField.value === 'modified') {
      const ta = a.modified_at ? new Date(a.modified_at).getTime() : 0;
      const tb = b.modified_at ? new Date(b.modified_at).getTime() : 0;
      res = ta - tb;
    }
    return sortAsc.value ? res : -res;
  });
  return list;
});

function handleDoubleClick(entry: VirtualArchiveEntry) {
  if (entry.kind === 'directory') {
    emit('navigate', entry.path);
  } else {
    emit('openPreview', entry);
  }
}

function calcSavedPercent(original: number, compressed?: number | null): number {
  if (!compressed || original <= 0 || compressed >= original) return 0;
  return Math.round(((original - compressed) / original) * 100);
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

function formatModifiedDate(isoString?: string | null): string {
  if (!isoString) return '—';
  try {
    const d = new Date(isoString);
    return d.toLocaleDateString(undefined, {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
    });
  } catch {
    return '—';
  }
}

function isArchiveFile(name: string): boolean {
  const ext = name.split('.').pop()?.toLowerCase() || '';
  return ['zip', 'tar', 'gz', 'tgz', '7z', 'bz2', 'xz'].includes(ext);
}

function isTextFile(name: string): boolean {
  if (name.startsWith('.')) return true;
  const ext = name.split('.').pop()?.toLowerCase() || '';
  const textExts = [
    'txt', 'md', 'log', 'env', 'json', 'yaml', 'yml', 'toml', 'xml', 'csv', 'tsv',
    'rs', 'ts', 'js', 'jsx', 'tsx', 'vue', 'html', 'css', 'scss', 'py', 'sh', 'c', 'cpp',
    'go', 'java', 'sql', 'conf', 'ini', 'dockerfile', 'lock'
  ];
  return textExts.includes(ext);
}

function getEntryIcon(entry: VirtualArchiveEntry): IconName {
  if (entry.kind === 'directory') return 'folder';
  const ext = entry.name.split('.').pop()?.toLowerCase() || '';
  if (['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp', 'ico'].includes(ext)) return 'image';
  if (['mp4', 'webm', 'mov', 'mkv', 'avi'].includes(ext)) return 'video';
  if (['mp3', 'wav', 'ogg', 'flac', 'm4a'].includes(ext)) return 'audio';
  if (isArchiveFile(entry.name)) return 'archive';
  if (['pdf'].includes(ext)) return 'pdf';
  if (isTextFile(entry.name)) return 'code';
  return 'file';
}

function getIconColorClass(entry: VirtualArchiveEntry): string {
  if (entry.kind === 'directory') return 'text-sky-500 dark:text-sky-400';
  const ext = entry.name.split('.').pop()?.toLowerCase() || '';
  if (['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp'].includes(ext)) return 'text-violet-500';
  if (['mp4', 'webm', 'mov'].includes(ext)) return 'text-pink-500';
  if (['mp3', 'wav', 'ogg'].includes(ext)) return 'text-emerald-500';
  if (isArchiveFile(entry.name)) return 'text-amber-500';
  if (['pdf'].includes(ext)) return 'text-rose-500';
  if (isTextFile(entry.name)) return 'text-cyan-500';
  return 'text-gray-400 dark:text-slate-400';
}

function getTypeLabel(entry: VirtualArchiveEntry): string {
  if (entry.kind === 'directory') return 'Folder';
  const ext = entry.name.split('.').pop()?.toUpperCase() || 'FILE';
  return ext;
}

function getTypeBadgeClass(entry: VirtualArchiveEntry): string {
  if (entry.kind === 'directory') {
    return 'bg-sky-500/10 text-sky-600 dark:text-sky-400';
  }
  const ext = entry.name.split('.').pop()?.toLowerCase() || '';
  if (['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp'].includes(ext)) {
    return 'bg-violet-500/10 text-violet-600 dark:text-violet-400';
  }
  if (['mp4', 'webm', 'mov'].includes(ext)) {
    return 'bg-pink-500/10 text-pink-600 dark:text-pink-400';
  }
  if (isArchiveFile(entry.name)) {
    return 'bg-amber-500/10 text-amber-600 dark:text-amber-400';
  }
  if (isTextFile(entry.name)) {
    return 'bg-cyan-500/10 text-cyan-600 dark:text-cyan-400';
  }
  return 'bg-gray-100 dark:bg-white/[0.06] text-gray-600 dark:text-slate-300';
}

function highlightMatch(text: string, query: string): string {
  if (!query) return text;
  const escaped = query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const regex = new RegExp(`(${escaped})`, 'gi');
  return text.replace(regex, '<mark class="bg-amber-400/40 dark:bg-amber-500/40 text-inherit rounded-xs px-0.5">$1</mark>');
}
</script>

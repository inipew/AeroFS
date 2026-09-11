<template>
  <div class="w-full">
    <table class="w-full text-left border-collapse text-xs select-none">
      <thead class="sticky top-0 z-10 bg-white/95 dark:bg-[#0b0f19]/95 backdrop-blur-xs border-b border-gray-200 dark:border-slate-800 text-[11px] font-bold text-gray-400 dark:text-slate-500 uppercase tracking-wider">
        <tr>
          <th class="py-2.5 px-3 w-8 text-center">
            <input
              type="checkbox"
              :checked="isAllSelected"
              @change="$emit('toggleSelectAll')"
              class="rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
            />
          </th>
          <th class="py-2.5 px-2 cursor-pointer hover:text-gray-900 dark:hover:text-white transition" @click="$emit('sort', 'name')">
            <div class="flex items-center space-x-1">
              <span>Name</span>
              <span v-if="sortField === 'name'" class="text-blue-600 dark:text-blue-400 text-xs">
                {{ sortOrder === 'asc' ? '▲' : '▼' }}
              </span>
            </div>
          </th>
          <th class="py-2.5 px-2 w-28 text-right cursor-pointer hover:text-gray-900 dark:hover:text-white transition" @click="$emit('sort', 'size')">
            <div class="flex items-center justify-end space-x-1">
              <span>Size</span>
              <span v-if="sortField === 'size'" class="text-blue-600 dark:text-blue-400 text-xs">
                {{ sortOrder === 'asc' ? '▲' : '▼' }}
              </span>
            </div>
          </th>
          <th class="py-2.5 px-2 w-36 text-right cursor-pointer hover:text-gray-900 dark:hover:text-white transition" @click="$emit('sort', 'modified')">
            <div class="flex items-center justify-end space-x-1">
              <span>Modified</span>
              <span v-if="sortField === 'modified'" class="text-blue-600 dark:text-blue-400 text-xs">
                {{ sortOrder === 'asc' ? '▲' : '▼' }}
              </span>
            </div>
          </th>
        </tr>
      </thead>
      <tbody class="divide-y divide-gray-100 dark:divide-slate-800/60 font-sans">
        <!-- Parent Directory Navigation Row (..) -->
        <tr
          v-if="currentPath !== '/' && currentPath !== ''"
          @click="$emit('navigateUp')"
          @dblclick="$emit('navigateUp')"
          class="cursor-pointer transition hover:bg-blue-50/40 dark:hover:bg-slate-800/60 text-gray-700 dark:text-slate-300"
          title="Go to parent directory (..)"
        >
          <td class="py-2.5 px-3 text-center"></td>
          <td class="py-2.5 px-2 flex items-center space-x-3 truncate">
            <FbIcon name="chevron-left" size="16px" class="text-blue-500 shrink-0" />
            <span class="font-bold text-sm text-blue-600 dark:text-blue-400">.. (Parent Directory)</span>
          </td>
          <td class="py-2.5 px-2 text-right text-gray-400 font-mono text-xs">-</td>
          <td class="py-2.5 px-2 text-right text-gray-400 font-mono text-xs">-</td>
        </tr>

        <!-- Virtual Spacer TOP -->
        <tr v-if="virtualItems.length > 0" aria-hidden="true">
          <td colspan="4" :style="{ height: `${offsetTop}px`, padding: 0 }"></td>
        </tr>

        <!-- Virtualized Rows -->
        <tr
          v-for="vRow in virtualItems"
          :key="entries[vRow.index]?.path ?? vRow.index"
          data-entry-item="true"
          :data-entry-path="entries[vRow.index]?.path"
          draggable="true"
          @dragstart="$emit('dragstart', $event, entries[vRow.index])"
          @touchstart.passive="$emit('entryTouchstart', $event, entries[vRow.index])"
          @touchend="$emit('entryTouchend')"
          @touchmove="$emit('entryTouchmove')"
          @touchcancel="$emit('entryTouchend')"
          @click="$emit('select', $event, entries[vRow.index])"
          @dblclick="$emit('activate', entries[vRow.index])"
          @contextmenu="$emit('contextmenu', $event, entries[vRow.index])"
          @dragover.stop.prevent="entries[vRow.index]?.kind === 'directory' ? $emit('folderDragover', $event, entries[vRow.index]) : null"
          @dragleave.stop="entries[vRow.index]?.kind === 'directory' ? $emit('folderDragleave', $event, entries[vRow.index]) : null"
          @drop.stop.prevent="entries[vRow.index]?.kind === 'directory' ? $emit('drop', $event, entries[vRow.index]) : null"
          :class="[
            'cursor-pointer transition group',
            entries[vRow.index] && isHidden(entries[vRow.index]) ? 'opacity-65 hover:opacity-100 italic' : '',
            entries[vRow.index] && isCut(entries[vRow.index].path) ? 'opacity-40 italic' : '',
            entries[vRow.index] && isDragged(entries[vRow.index].path) ? 'opacity-40 border-dashed border-blue-400 scale-[0.99]' : '',
            entries[vRow.index] && hoveredFolderDrop === entries[vRow.index].path
              ? 'bg-blue-100/90 dark:bg-blue-900/70 border-l-4 border-l-blue-600 ring-2 ring-blue-500/40 shadow-md'
              : (entries[vRow.index] && selectedPaths.includes(entries[vRow.index].path)
                ? 'bg-blue-50/80 dark:bg-blue-950/40 text-blue-900 dark:text-blue-200 border-l-2 border-l-blue-600 dark:border-l-blue-400'
                : 'hover:bg-gray-50/80 dark:hover:bg-slate-800/60 text-gray-800 dark:text-slate-200 border-l-2 border-l-transparent')
          ]"
        >
          <td
            :class="[
              density === 'comfortable' ? 'py-3' : (density === 'dense' ? 'py-1' : 'py-2'),
              'px-3 text-center'
            ]"
            @click.stop
          >
            <input
              v-if="entries[vRow.index]"
              type="checkbox"
              :checked="selectedPaths.includes(entries[vRow.index].path)"
              @change="$emit('toggleSelect', entries[vRow.index].path)"
              class="rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
            />
          </td>
          <td
            :class="[
              density === 'comfortable' ? 'py-3' : (density === 'dense' ? 'py-1' : 'py-2'),
              'px-2 flex items-center space-x-3 truncate'
            ]"
          >
            <template v-if="entries[vRow.index]">
              <FbIcon
                :name="entries[vRow.index].kind === 'directory' ? 'folder' : getCategoryIcon(entries[vRow.index])"
                :size="density === 'dense' ? '15px' : '18px'"
                :class="[
                  isHidden(entries[vRow.index])
                    ? 'text-gray-400 dark:text-slate-500'
                    : (entries[vRow.index].kind === 'directory'
                      ? 'text-blue-600 dark:text-blue-400'
                      : getCategoryIconColor(entries[vRow.index]))
                ]"
              />
              <div class="truncate flex items-center space-x-1.5 min-w-0">
                <span
                  class="truncate font-medium group-hover:text-blue-600 dark:group-hover:text-blue-400 transition"
                  :class="[
                    entries[vRow.index].kind === 'directory' ? 'font-semibold' : '',
                    density === 'dense' ? 'text-xs' : 'text-sm'
                  ]"
                >
                  {{ entries[vRow.index].name }}
                </span>
                <span v-if="isHidden(entries[vRow.index])" class="text-[9px] px-1.5 py-0.2 rounded bg-gray-200/80 dark:bg-slate-800 text-gray-400 dark:text-slate-500 font-mono not-italic">
                  dot
                </span>
                <span
                  v-if="hoveredFolderDrop === entries[vRow.index].path"
                  class="text-[10px] font-semibold px-2 py-0.5 rounded-full bg-blue-600 text-white shadow-xs animate-pulse inline-flex items-center space-x-1 shrink-0 ml-1.5"
                >
                  Drop into folder
                </span>
              </div>
            </template>
          </td>
          <td
            :class="[
              density === 'comfortable' ? 'py-3' : (density === 'dense' ? 'py-1' : 'py-2'),
              'px-2 text-right text-gray-500 dark:text-slate-400 font-mono text-xs'
            ]"
          >
            <template v-if="entries[vRow.index]">
              <span v-if="entries[vRow.index].kind === 'directory'" class="text-gray-300 dark:text-slate-600">-</span>
              <span v-else>{{ formatBytes(entries[vRow.index].size || 0) }}</span>
            </template>
          </td>
          <td
            :class="[
              density === 'comfortable' ? 'py-3' : (density === 'dense' ? 'py-1' : 'py-2'),
              'px-2 text-right text-gray-500 dark:text-slate-400 text-xs'
            ]"
          >
            <template v-if="entries[vRow.index]">
              {{ formatDate(entries[vRow.index].modified_at) }}
            </template>
          </td>
        </tr>

        <!-- Virtual Spacer BOTTOM -->
        <tr v-if="virtualItems.length > 0" aria-hidden="true">
          <td colspan="4" :style="{ height: `${offsetBottom}px`, padding: 0 }"></td>
        </tr>
      </tbody>
    </table>

    <!-- Load More button for List Table View -->
    <div v-if="hasMore" class="p-4 text-center border-t border-gray-100 dark:border-slate-800">
      <button
        type="button"
        @click.stop="$emit('loadMore')"
        :disabled="isFetchingNextPage"
        class="px-5 py-2 rounded-xl bg-blue-50 dark:bg-slate-800 hover:bg-blue-100 dark:hover:bg-slate-700 text-blue-600 dark:text-blue-400 text-xs font-semibold transition cursor-pointer disabled:opacity-50 inline-flex items-center space-x-2 shadow-xs"
      >
        <div v-if="isFetchingNextPage" class="animate-spin rounded-full h-3.5 w-3.5 border-2 border-blue-500 border-t-transparent"></div>
        <span>{{ isFetchingNextPage ? 'Loading more...' : 'Load More Files' }}</span>
        <span v-if="totalCount" class="text-gray-400 dark:text-slate-500 text-[10px]">({{ entries.length }} of {{ totalCount }})</span>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useVirtualizer } from '@tanstack/vue-virtual';
import type { FileEntry } from '../../../types/vfs';
import FbIcon from '../../../components/common/FbIcon.vue';
import { formatBytes, formatDate } from '../../../utils/formatters';
import { getFileExt, isTextOrCode } from '../../../utils/fileTypes';
import type { IconName } from '../../../utils/icons';

const props = withDefaults(
  defineProps<{
    entries: FileEntry[];
    currentPath: string;
    selectedPaths: string[];
    cutPaths?: string[];
    draggedPaths?: string[];
    hoveredFolderDrop?: string | null;
    density?: 'dense' | 'compact' | 'normal' | 'comfortable';
    sortField?: string;
    sortOrder?: 'asc' | 'desc';
    isAllSelected?: boolean;
    scrollElement: HTMLElement | null;
    hasMore?: boolean;
    isFetchingNextPage?: boolean;
    totalCount?: number | null;
  }>(),
  {
    cutPaths: () => [],
    draggedPaths: () => [],
    hoveredFolderDrop: null,
    density: 'normal',
    sortField: 'name',
    sortOrder: 'asc',
    isAllSelected: false,
    hasMore: false,
    isFetchingNextPage: false,
    totalCount: 0,
  }
);

defineEmits<{
  (e: 'select', event: MouseEvent, entry: FileEntry): void;
  (e: 'activate', entry: FileEntry): void;
  (e: 'toggleSelect', path: string): void;
  (e: 'toggleSelectAll'): void;
  (e: 'navigateUp'): void;
  (e: 'sort', field: string): void;
  (e: 'contextmenu', event: MouseEvent, entry: FileEntry): void;
  (e: 'dragstart', event: DragEvent, entry: FileEntry): void;
  (e: 'folderDragover', event: DragEvent, folder: FileEntry): void;
  (e: 'folderDragleave', event: DragEvent, folder: FileEntry): void;
  (e: 'drop', event: DragEvent, folder: FileEntry): void;
  (e: 'entryTouchstart', event: TouchEvent, entry: FileEntry): void;
  (e: 'entryTouchmove'): void;
  (e: 'entryTouchend'): void;
  (e: 'loadMore'): void;
}>();

const virtualizer = useVirtualizer({
  get count() {
    return props.entries.length;
  },
  getScrollElement: () => props.scrollElement,
  estimateSize: () =>
    props.density === 'dense' || props.density === 'compact' ? 32 : props.density === 'comfortable' ? 44 : 38,
  overscan: 10,
});

const virtualItems = computed(() => virtualizer.value.getVirtualItems());
const totalSize = computed(() => virtualizer.value.getTotalSize());
const offsetTop = computed(() => virtualItems.value[0]?.start ?? 0);
const offsetBottom = computed(() => {
  const last = virtualItems.value.at(-1);
  if (!last) return 0;
  return totalSize.value - last.end;
});

function isHidden(entry: FileEntry): boolean {
  return entry.is_hidden || entry.name.startsWith('.');
}

function isCut(path: string): boolean {
  return props.cutPaths.includes(path);
}

function isDragged(path: string): boolean {
  return (props.draggedPaths || []).includes(path);
}

function getCategoryIcon(entry: FileEntry): IconName {
  const ext = getFileExt(entry);
  if (['zip', 'tar', 'gz', 'tgz', '7z', 'rar', 'bz2', 'xz'].includes(ext)) return 'archive';
  if (['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'ico', 'bmp', 'avif', 'tiff', 'heic'].includes(ext)) return 'image';
  if (['mp4', 'webm', 'mov', 'mkv', 'avi', 'flv', 'wmv', 'm4v', '3gp', 'ogv'].includes(ext)) return 'video';
  if (['mp3', 'wav', 'ogg', 'flac', 'm4a', 'opus', 'aac', 'wma'].includes(ext)) return 'audio';
  if (['pdf'].includes(ext)) return 'pdf';
  if (isTextOrCode(entry)) return 'code';
  return 'file';
}

function getCategoryIconColor(entry: FileEntry): string {
  const ext = getFileExt(entry);
  if (['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'ico', 'bmp', 'avif', 'tiff', 'heic'].includes(ext)) {
    return 'text-emerald-500 dark:text-emerald-400';
  }
  if (['mp4', 'webm', 'mov', 'mkv', 'avi', 'flv', 'wmv', 'm4v', '3gp', 'ogv'].includes(ext)) {
    return 'text-rose-500 dark:text-rose-400';
  }
  if (['mp3', 'wav', 'ogg', 'flac', 'm4a', 'opus', 'aac', 'wma'].includes(ext)) {
    return 'text-violet-500 dark:text-violet-400';
  }
  if (['zip', 'tar', 'gz', 'tgz', '7z', 'rar', 'bz2', 'xz'].includes(ext)) {
    return 'text-amber-500 dark:text-amber-400';
  }
  if (['pdf'].includes(ext)) {
    return 'text-red-500 dark:text-red-400';
  }
  if (isTextOrCode(entry)) {
    return 'text-blue-500 dark:text-blue-400';
  }
  return 'text-gray-400 dark:text-slate-500';
}
</script>

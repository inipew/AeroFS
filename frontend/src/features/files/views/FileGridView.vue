<template>
  <div class="w-full">
    <!-- Parent Folder Navigation Card (..) -->
    <div
      v-if="currentPath !== '/' && currentPath !== ''"
      class="mb-3"
    >
      <div
        @click="$emit('navigateUp')"
        @dblclick="$emit('navigateUp')"
        class="w-32 border border-dashed border-gray-300/80 dark:border-slate-700/80 hover:border-blue-500 dark:hover:border-blue-400 rounded-2xl p-3 flex flex-col items-center justify-between text-center cursor-pointer transition-[transform,background-color,border-color,box-shadow] duration-standard ease-spring select-none shadow-xs group bg-gray-50/60 dark:bg-slate-900/40 hover:bg-blue-50/40 dark:hover:bg-blue-950/30 hover:-translate-y-0.5 active:scale-[0.98] min-h-[100px]"
        title="Go to parent directory (..)"
      >
        <div class="flex-1 flex items-center justify-center w-full py-1">
          <div class="w-9 h-9 rounded-2xl bg-gradient-to-tr from-blue-500/10 to-indigo-500/10 dark:from-blue-500/20 dark:to-indigo-500/20 text-blue-600 dark:text-blue-400 flex items-center justify-center group-hover:scale-110 transition-transform duration-standard ease-spring ring-1 ring-blue-500/20">
            <FbIcon name="arrow-up" size="16px" class="group-hover:-translate-y-0.5 transition-transform" />
          </div>
        </div>
        <span class="font-bold text-xs truncate text-gray-700 dark:text-slate-300 group-hover:text-blue-600 dark:group-hover:text-blue-400 w-full block">.. Parent</span>
      </div>
    </div>

    <!-- Virtual Grid Container -->
    <div
      v-if="entries.length > 0"
      :style="{ height: `${gridTotalSize}px`, position: 'relative' }"
    >
      <div
        v-for="vRow in virtualGridRows"
        :key="String(vRow.key)"
        :style="{
          position: 'absolute',
          top: 0,
          left: 0,
          width: '100%',
          height: `${vRow.size - 12}px`,
          transform: `translateY(${vRow.start}px)`,
          gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`,
        }"
        class="grid gap-3 sm:gap-3.5"
      >
        <div
          v-for="colIdx in columns"
          :key="colIdx"
          class="h-full"
        >
          <template v-if="getItemAt(vRow.index, colIdx - 1)">
            <!-- FOLDER CARD -->
            <div
              v-if="getItemAt(vRow.index, colIdx - 1)!.kind === 'directory'"
              data-entry-item="true"
              :data-entry-path="getItemAt(vRow.index, colIdx - 1)!.path"
              draggable="true"
              @dragstart="$emit('dragstart', $event, getItemAt(vRow.index, colIdx - 1)!)"
              @touchstart.passive="$emit('entryTouchstart', $event, getItemAt(vRow.index, colIdx - 1)!)"
              @touchend="$emit('entryTouchend')"
              @touchmove="$emit('entryTouchmove')"
              @touchcancel="$emit('entryTouchend')"
              @click="$emit('select', $event, getItemAt(vRow.index, colIdx - 1)!)"
              @dblclick="$emit('activate', getItemAt(vRow.index, colIdx - 1)!)"
              @contextmenu="$emit('contextmenu', $event, getItemAt(vRow.index, colIdx - 1)!)"
              @dragover.stop.prevent="$emit('folderDragover', $event, getItemAt(vRow.index, colIdx - 1)!)"
              @dragleave.stop="$emit('folderDragleave', $event, getItemAt(vRow.index, colIdx - 1)!)"
              @drop.stop.prevent="$emit('drop', $event, getItemAt(vRow.index, colIdx - 1)!)"
              :class="[
                'border rounded-2xl p-2.5 flex flex-col items-center justify-between text-center cursor-pointer transition-[transform,background-color,border-color,box-shadow] duration-standard ease-spring select-none shadow-xs group active:scale-[0.98] h-full overflow-hidden relative',
                isHidden(getItemAt(vRow.index, colIdx - 1)!) ? 'opacity-65 hover:opacity-100 border-dashed border-gray-300 dark:border-slate-700 bg-gray-50/50 dark:bg-slate-900/40' : '',
                isCut(getItemAt(vRow.index, colIdx - 1)!.path) ? 'opacity-40 border-dashed border-amber-500 ring-1 ring-amber-500/30' : '',
                isDragged(getItemAt(vRow.index, colIdx - 1)!.path) ? 'opacity-40 border-dashed border-blue-400 scale-[0.96]' : '',
                hoveredFolderDrop === getItemAt(vRow.index, colIdx - 1)!.path
                  ? 'ring-2 ring-blue-500 scale-[1.06] bg-blue-100/80 dark:bg-blue-900/70 border-blue-500 shadow-xl shadow-blue-500/20 z-10'
                  : (selectedPaths.includes(getItemAt(vRow.index, colIdx - 1)!.path)
                    ? 'bg-blue-50/80 dark:bg-blue-950/50 border-blue-500 ring-2 ring-blue-500/30 shadow-md'
                    : 'bg-white dark:bg-[#0f1422] border-gray-200/90 dark:border-slate-800/90 hover:border-blue-400 dark:hover:border-blue-500 hover:shadow-lg hover:shadow-blue-500/5 hover:-translate-y-1')
              ]"
            >
              <div class="flex-1 flex items-center justify-center w-full py-1 relative">
                <svg viewBox="0 0 64 64" fill="none" xmlns="http://www.w3.org/2000/svg" class="w-12 h-12 sm:w-13 sm:h-13 drop-shadow-xs group-hover:scale-110 transition-transform duration-standard ease-spring" :class="{ 'scale-110': hoveredFolderDrop === getItemAt(vRow.index, colIdx - 1)!.path }">
                  <path d="M6 18C6 14.6863 8.68629 12 12 12H24.3431C25.9345 12 27.4609 12.6321 28.5858 13.7574L32.4142 17.5858C33.5391 18.7107 35.0655 19.3431 36.6569 19.3431H52C55.3137 19.3431 58 22.0294 58 25.3431V46C58 49.3137 55.3137 52 52 52H12C8.68629 52 6 49.3137 6 46V18Z" class="fill-sky-500 dark:fill-sky-600" />
                  <path d="M6 25C6 21.6863 8.68629 19 12 19H52C55.3137 19 58 21.6863 58 25V46C58 49.3137 55.3137 52 52 52H12C8.68629 52 6 49.3137 6 46V25Z" class="fill-sky-400 dark:fill-sky-400" />
                </svg>
                <span
                  v-if="hoveredFolderDrop === getItemAt(vRow.index, colIdx - 1)!.path"
                  class="absolute bottom-0 text-[10px] font-bold px-2 py-0.5 rounded-full bg-blue-600 text-white shadow-md animate-bounce select-none pointer-events-none whitespace-nowrap z-20"
                >
                  Drop inside
                </span>
              </div>
              <div class="w-full px-0.5 mt-1 text-center shrink-0">
                <span class="font-semibold text-xs text-gray-800 dark:text-slate-100 group-hover:text-blue-600 dark:group-hover:text-blue-400 transition line-clamp-2 break-all leading-tight block" :title="getItemAt(vRow.index, colIdx - 1)!.name">
                  {{ getItemAt(vRow.index, colIdx - 1)!.name }}
                </span>
                <span v-if="isHidden(getItemAt(vRow.index, colIdx - 1)!)" class="inline-block mt-0.5 text-[8px] px-1 py-0.2 rounded bg-gray-200/80 dark:bg-slate-800 text-gray-400 dark:text-slate-500 font-mono">
                  dot
                </span>
              </div>
            </div>

            <!-- FILE CARD -->
            <div
              v-else
              data-entry-item="true"
              :data-entry-path="getItemAt(vRow.index, colIdx - 1)!.path"
              draggable="true"
              @dragstart="$emit('dragstart', $event, getItemAt(vRow.index, colIdx - 1)!)"
              @touchstart.passive="$emit('entryTouchstart', $event, getItemAt(vRow.index, colIdx - 1)!)"
              @touchend="$emit('entryTouchend')"
              @touchmove="$emit('entryTouchmove')"
              @touchcancel="$emit('entryTouchend')"
              @click="$emit('select', $event, getItemAt(vRow.index, colIdx - 1)!)"
              @dblclick="$emit('activate', getItemAt(vRow.index, colIdx - 1)!)"
              @contextmenu="$emit('contextmenu', $event, getItemAt(vRow.index, colIdx - 1)!)"
              :class="[
                'border rounded-2xl overflow-hidden cursor-pointer transition-[transform,background-color,border-color,box-shadow] duration-standard ease-spring flex flex-col group select-none shadow-xs active:scale-[0.98] h-full',
                isHidden(getItemAt(vRow.index, colIdx - 1)!) ? 'opacity-65 hover:opacity-100 border-dashed border-gray-300 dark:border-slate-700 bg-gray-50/30 dark:bg-slate-900/30' : '',
                isCut(getItemAt(vRow.index, colIdx - 1)!.path) ? 'opacity-40 border-dashed border-amber-500 ring-1 ring-amber-500/30' : '',
                isDragged(getItemAt(vRow.index, colIdx - 1)!.path) ? 'opacity-40 border-dashed border-blue-400 scale-[0.96]' : '',
                selectedPaths.includes(getItemAt(vRow.index, colIdx - 1)!.path)
                  ? 'bg-blue-50/80 dark:bg-blue-950/50 border-blue-500 ring-2 ring-blue-500/30 shadow-md'
                  : 'bg-white dark:bg-[#0f1422] border-gray-200/90 dark:border-slate-800/90 hover:shadow-lg hover:shadow-blue-500/5 hover:-translate-y-1 hover:border-blue-400 dark:hover:border-blue-500'
              ]"
            >
              <div class="flex-1 w-full bg-slate-50/80 dark:bg-slate-950/70 relative overflow-hidden border-b border-gray-100 dark:border-slate-800/80 min-h-0">
                <!-- Image Thumbnail -->
                <template v-if="isImage(getItemAt(vRow.index, colIdx - 1)!) && !failedThumbnails.has(getItemAt(vRow.index, colIdx - 1)!.path)">
                  <div class="absolute inset-0 p-2 flex items-center justify-center">
                    <img
                      :src="getDownloadUrl(connectionId, getItemAt(vRow.index, colIdx - 1)!.path)"
                      :alt="getItemAt(vRow.index, colIdx - 1)!.name"
                      @error="handleThumbnailError(getItemAt(vRow.index, colIdx - 1)!.path)"
                      class="max-w-full max-h-full object-contain rounded drop-shadow-xs group-hover:scale-105 transition duration-300 pointer-events-none select-none"
                      loading="lazy"
                    />
                  </div>
                  <span class="absolute bottom-1.5 right-1.5 text-[8px] px-1 py-0.2 rounded-md bg-black/75 backdrop-blur-xs text-white/90 font-mono font-bold uppercase tracking-wider shadow-md border border-white/10 select-none pointer-events-none">
                    {{ getFileExt(getItemAt(vRow.index, colIdx - 1)!) }}
                  </span>
                </template>

                <!-- Video Thumbnail with Play Button Ring -->
                <template v-else-if="isVideo(getItemAt(vRow.index, colIdx - 1)!) && !failedThumbnails.has(getItemAt(vRow.index, colIdx - 1)!.path)">
                  <div class="absolute inset-0 p-2 flex items-center justify-center">
                    <video
                      :src="getDownloadUrl(connectionId, getItemAt(vRow.index, colIdx - 1)!.path) + '#t=0.5'"
                      preload="metadata"
                      muted
                      playsinline
                      @error="handleThumbnailError(getItemAt(vRow.index, colIdx - 1)!.path)"
                      class="max-w-full max-h-full object-contain rounded group-hover:scale-105 transition duration-300 pointer-events-none"
                    ></video>
                    <div class="absolute inset-0 flex items-center justify-center pointer-events-none">
                      <div class="w-8 h-8 rounded-full bg-black/60 backdrop-blur-md flex items-center justify-center text-white ring-1 ring-white/40 group-hover:scale-110 group-hover:bg-blue-600 transition duration-200 shadow-xl pl-0.5">
                        <FbIcon name="play" size="12px" class="fill-white" />
                      </div>
                    </div>
                  </div>
                  <span class="absolute bottom-1.5 right-1.5 text-[8px] px-1 py-0.2 rounded-md bg-black/75 backdrop-blur-xs text-white/90 font-mono font-bold uppercase tracking-wider shadow-md z-10 border border-white/10 select-none pointer-events-none">
                    {{ getFileExt(getItemAt(vRow.index, colIdx - 1)!) }}
                  </span>
                </template>

                <!-- Audio with Art & Wave Badge -->
                <template v-else-if="isAudio(getItemAt(vRow.index, colIdx - 1)!)">
                  <div class="absolute inset-0 p-2 flex items-center justify-center">
                    <div class="w-full h-full bg-gradient-to-br from-violet-500/15 via-purple-500/15 to-pink-500/15 dark:from-violet-950/50 dark:to-purple-950/50 flex flex-col items-center justify-center space-y-1 rounded-lg">
                      <div class="w-8 h-8 rounded-xl bg-gradient-to-tr from-violet-600 to-indigo-600 flex items-center justify-center text-white text-sm shadow-md group-hover:scale-110 transition duration-200">
                        🎵
                      </div>
                      <span class="text-[8px] font-mono font-bold uppercase text-violet-600 dark:text-violet-400 tracking-wider">
                        {{ getFileExt(getItemAt(vRow.index, colIdx - 1)!) }}
                      </span>
                    </div>
                  </div>
                </template>

                <!-- Document Sheet Fallback with Soft Glow Halo & Sleek Miniature Sheet Paper -->
                <div v-else class="absolute inset-0 flex flex-col items-center justify-center">
                  <div :class="['absolute inset-0 bg-gradient-to-b opacity-40 dark:opacity-30 pointer-events-none rounded-t-2xl', getFileTypeMeta(getItemAt(vRow.index, colIdx - 1)!).cardBg]"></div>
                  <div class="relative flex flex-col items-center justify-center group-hover:scale-105 transition-transform duration-200 ease-spring">
                    <!-- Soft glow halo -->
                    <div :class="['absolute w-12 h-12 rounded-full blur-md opacity-25 dark:opacity-40 pointer-events-none', getFileTypeMeta(getItemAt(vRow.index, colIdx - 1)!).badgeBg]"></div>

                    <!-- Sleek Document Sheet -->
                    <div
                      class="w-11 h-14 sm:w-12 sm:h-15 rounded-xl relative flex flex-col items-center justify-between p-1.5 shadow-xs border bg-white/95 dark:bg-[#131b2e]/95 backdrop-blur-xs z-10 transition-shadow duration-200"
                      :class="getFileTypeMeta(getItemAt(vRow.index, colIdx - 1)!).badgeBorder"
                    >
                      <!-- Centered File Symbol -->
                      <div class="flex-1 flex items-center justify-center">
                        <span class="text-base select-none filter drop-shadow-xs">{{ getFileTypeMeta(getItemAt(vRow.index, colIdx - 1)!).symbol }}</span>
                      </div>
                      <!-- Extension Pill Badge -->
                      <span
                        class="w-full py-0.5 px-1 rounded-md font-mono text-[8.5px] sm:text-[9px] font-bold uppercase tracking-wider border shadow-2xs truncate text-center leading-none"
                        :class="[getFileTypeMeta(getItemAt(vRow.index, colIdx - 1)!).badgeBg, getFileTypeMeta(getItemAt(vRow.index, colIdx - 1)!).badgeText, getFileTypeMeta(getItemAt(vRow.index, colIdx - 1)!).badgeBorder]"
                      >
                        {{ getFileTypeMeta(getItemAt(vRow.index, colIdx - 1)!).label }}
                      </span>
                    </div>
                  </div>
                </div>
              </div>

              <!-- File Name & Meta Footer -->
              <div class="p-2 w-full text-left shrink-0 bg-white dark:bg-[#0f1422]">
                <div class="text-xs font-semibold text-gray-800 dark:text-slate-100 group-hover:text-blue-600 dark:group-hover:text-blue-400 transition truncate" :title="getItemAt(vRow.index, colIdx - 1)!.name">
                  {{ getItemAt(vRow.index, colIdx - 1)!.name }}
                </div>
                <div class="text-[10px] text-gray-400 dark:text-slate-500 mt-0.5 font-normal font-mono truncate w-full">
                  {{ formatBytes(getItemAt(vRow.index, colIdx - 1)!.size || 0) }}
                </div>
              </div>
            </div>
          </template>
        </div>
      </div>
    </div>

    <!-- Load More button for Grid View -->
    <div v-if="hasMore" class="mt-6 mb-4 text-center">
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
import { ref, computed } from 'vue';
import { useVirtualizer } from '@tanstack/vue-virtual';
import type { FileEntry } from '../../../types/vfs';
import FbIcon from '../../../components/common/FbIcon.vue';
import { formatBytes } from '../../../utils/formatters';
import { getFileExt, getFileTypeMeta } from '../../../utils/fileTypes';
import { getDownloadUrl } from '../../../api/files';

const props = withDefaults(
  defineProps<{
    entries: FileEntry[];
    currentPath: string;
    connectionId: string;
    selectedPaths: string[];
    cutPaths?: string[];
    draggedPaths?: string[];
    hoveredFolderDrop?: string | null;
    columns: number;
    scrollElement: HTMLElement | null;
    hasMore?: boolean;
    isFetchingNextPage?: boolean;
    totalCount?: number | null;
  }>(),
  {
    cutPaths: () => [],
    draggedPaths: () => [],
    hoveredFolderDrop: null,
    columns: 4,
    hasMore: false,
    isFetchingNextPage: false,
    totalCount: 0,
  }
);

defineEmits<{
  (e: 'select', event: MouseEvent, entry: FileEntry): void;
  (e: 'activate', entry: FileEntry): void;
  (e: 'navigateUp'): void;
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

const failedThumbnails = ref<Set<string>>(new Set());

function handleThumbnailError(path: string) {
  failedThumbnails.value.add(path);
}

const gridRowCount = computed(() => Math.ceil(props.entries.length / props.columns));

const gridVirtualizer = useVirtualizer({
  get count() {
    return gridRowCount.value;
  },
  getScrollElement: () => props.scrollElement,
  estimateSize: () => 156,
  overscan: 3,
});

const virtualGridRows = computed(() => gridVirtualizer.value.getVirtualItems());
const gridTotalSize = computed(() => gridVirtualizer.value.getTotalSize());

function getItemAt(rowIndex: number, colIdx: number): FileEntry | null {
  const idx = rowIndex * props.columns + colIdx;
  return props.entries[idx] ?? null;
}

function isHidden(entry: FileEntry): boolean {
  return entry.is_hidden || entry.name.startsWith('.');
}

function isCut(path: string): boolean {
  return props.cutPaths.includes(path);
}

function isDragged(path: string): boolean {
  return (props.draggedPaths || []).includes(path);
}

function isImage(entry: FileEntry): boolean {
  const ext = getFileExt(entry);
  return ['jpg', 'jpeg', 'png', 'gif', 'webp', 'svg', 'ico', 'bmp', 'avif', 'tiff', 'heic'].includes(ext);
}

function isAudio(entry: FileEntry): boolean {
  const ext = getFileExt(entry);
  return ['mp3', 'wav', 'flac', 'aac', 'm4a', 'opus', 'ogg', 'wma'].includes(ext);
}

function isVideo(entry: FileEntry): boolean {
  const ext = getFileExt(entry);
  return ['mp4', 'webm', 'mov', 'avi', 'mkv', 'flv', 'wmv', 'm4v', '3gp', 'ogv'].includes(ext);
}
</script>

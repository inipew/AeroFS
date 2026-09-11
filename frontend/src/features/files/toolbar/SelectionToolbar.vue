<template>
  <div class="flex items-center justify-between w-full h-full">
    <!-- Left: Deselect Arrow & Selection Count Badge -->
    <div class="flex items-center space-x-2 sm:space-x-3 min-w-0">
      <button
        type="button"
        @click="$emit('deselect')"
        class="p-1.5 sm:p-2 rounded-xl text-gray-500 hover:text-gray-900 dark:text-slate-400 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer active:scale-95 duration-fast ease-spring"
        title="Deselect All (Esc)"
      >
        <FbIcon name="arrow-back" size="16px" />
      </button>

      <div class="flex items-center space-x-1.5 sm:space-x-2 px-2.5 py-1 rounded-xl bg-blue-50 dark:bg-blue-950/60 border border-blue-200/80 dark:border-blue-800/60 text-xs font-bold text-blue-600 dark:text-blue-400 shadow-2xs truncate">
        <span class="w-2 h-2 rounded-full bg-blue-600 dark:bg-blue-400 animate-pulse shrink-0"></span>
        <span>{{ selectedCount }} Selected</span>
        <span v-if="selectedTotalSize > 0" class="text-[11px] font-mono font-normal opacity-75 hidden sm:inline">
          ({{ formattedSize }})
        </span>
      </div>
    </div>

    <!-- Right: Contextual Action Pills -->
    <div class="flex items-center space-x-1 sm:space-x-1.5 shrink-0">
      <!-- Compress / Archive -->
      <button
        type="button"
        @click="$emit('compress')"
        class="px-2 sm:px-2.5 py-1 sm:py-1.5 rounded-xl border border-gray-200 dark:border-slate-700 bg-white dark:bg-slate-900 hover:bg-gray-50 dark:hover:bg-slate-800 text-xs font-medium text-gray-700 dark:text-slate-200 flex items-center space-x-1.5 transition active:scale-95 cursor-pointer shadow-2xs"
        title="Compress selected items into archive"
      >
        <FbIcon name="archive" size="13px" class="text-amber-500" />
        <span class="hidden md:inline">Compress</span>
      </button>

      <!-- Rename (Single Selection) -->
      <button
        v-if="singleSelected"
        type="button"
        @click="$emit('rename')"
        class="px-2 sm:px-2.5 py-1 sm:py-1.5 rounded-xl border border-gray-200 dark:border-slate-700 bg-white dark:bg-slate-900 hover:bg-gray-50 dark:hover:bg-slate-800 text-xs font-medium text-gray-700 dark:text-slate-200 flex items-center space-x-1.5 transition active:scale-95 cursor-pointer shadow-2xs"
        title="Rename item (F2)"
      >
        <FbIcon name="rename" size="13px" class="text-blue-500" />
        <span class="hidden md:inline">Rename</span>
      </button>

      <!-- Copy Selection -->
      <button
        type="button"
        @click="$emit('copy')"
        class="px-2 sm:px-2.5 py-1 sm:py-1.5 rounded-xl border border-gray-200 dark:border-slate-700 bg-white dark:bg-slate-900 hover:bg-gray-50 dark:hover:bg-slate-800 text-xs font-medium text-gray-700 dark:text-slate-200 flex items-center space-x-1.5 transition active:scale-95 cursor-pointer shadow-2xs"
        title="Copy selected items (Ctrl+C)"
      >
        <FbIcon name="copy" size="13px" class="text-emerald-500" />
        <span class="hidden md:inline">Copy</span>
      </button>

      <!-- Cut Selection -->
      <button
        type="button"
        @click="$emit('cut')"
        class="px-2 sm:px-2.5 py-1 sm:py-1.5 rounded-xl border border-gray-200 dark:border-slate-700 bg-white dark:bg-slate-900 hover:bg-gray-50 dark:hover:bg-slate-800 text-xs font-medium text-gray-700 dark:text-slate-200 flex items-center space-x-1.5 transition active:scale-95 cursor-pointer shadow-2xs"
        title="Cut selected items (Ctrl+X)"
      >
        <FbIcon name="move" size="13px" class="text-indigo-500" />
        <span class="hidden md:inline">Cut</span>
      </button>

      <!-- Delete Action -->
      <button
        type="button"
        @click="$emit('delete')"
        class="px-2 sm:px-2.5 py-1 sm:py-1.5 rounded-xl border border-rose-200 dark:border-rose-900/60 bg-rose-50/80 dark:bg-rose-950/50 hover:bg-rose-100 dark:hover:bg-rose-900/60 text-xs font-semibold text-rose-600 dark:text-rose-400 flex items-center space-x-1.5 transition active:scale-95 cursor-pointer shadow-2xs"
        title="Delete selected items (Del)"
      >
        <FbIcon name="delete" size="13px" />
        <span>Delete</span>
      </button>

      <div class="h-4 w-px bg-gray-200 dark:bg-slate-800 mx-0.5"></div>

      <!-- Clear / Close (✕) -->
      <button
        type="button"
        @click="$emit('deselect')"
        class="p-1.5 sm:p-2 rounded-xl text-gray-400 hover:text-gray-700 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer active:scale-95 duration-fast ease-spring text-xs font-bold"
        title="Done / Deselect All (Esc)"
      >
        ✕
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import FbIcon from '../../../components/common/FbIcon.vue';
import { formatBytes } from '../../../utils/formatters';

const props = withDefaults(
  defineProps<{
    selectedCount: number;
    selectedTotalSize?: number;
    singleSelected?: boolean;
  }>(),
  {
    selectedTotalSize: 0,
    singleSelected: false,
  }
);

defineEmits<{
  (e: 'deselect'): void;
  (e: 'compress'): void;
  (e: 'rename'): void;
  (e: 'copy'): void;
  (e: 'cut'): void;
  (e: 'delete'): void;
}>();

const formattedSize = computed(() => formatBytes(props.selectedTotalSize));
</script>

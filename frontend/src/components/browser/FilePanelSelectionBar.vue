<template>
  <Transition name="ios-selection-bar">
    <div
      v-if="selectedCount > 0"
      class="absolute bottom-11 inset-x-3 sm:inset-x-auto sm:left-1/2 sm:-translate-x-1/2 z-40 bg-white/95 dark:bg-[#0d1424]/95 backdrop-blur-md border border-blue-500/30 dark:border-blue-500/40 rounded-2xl shadow-2xl px-3.5 py-2 flex items-center justify-between sm:justify-center gap-3 text-xs font-semibold text-gray-800 dark:text-slate-100 select-none pointer-events-auto"
    >
      <div class="flex items-center space-x-2 shrink-0">
        <span class="w-2 h-2 rounded-full bg-blue-600 animate-pulse"></span>
        <span class="font-bold text-blue-600 dark:text-blue-400">{{ selectedCount }} selected</span>
        <span v-if="selectedSize > 0" class="text-[11px] font-mono text-gray-500 dark:text-slate-400 font-normal">
          ({{ formatBytes(selectedSize) }})
        </span>
      </div>

      <div class="h-4 w-px bg-gray-200 dark:bg-slate-700 hidden sm:block"></div>

      <div class="flex items-center space-x-1 sm:space-x-1.5 shrink-0">
        <button
          @click.stop="$emit('compress')"
          class="px-2.5 py-1.5 rounded-xl bg-gray-100 hover:bg-blue-100 dark:bg-slate-800 dark:hover:bg-slate-700 text-gray-700 dark:text-slate-200 border border-gray-200 dark:border-slate-700 flex items-center space-x-1 transition cursor-pointer text-xs"
          title="Compress selected items"
        >
          <FbIcon name="archive" size="13px" />
          <span class="hidden sm:inline">Compress</span>
        </button>

        <button
          v-if="singleSelected"
          @click.stop="$emit('rename')"
          class="px-2.5 py-1.5 rounded-xl bg-gray-100 hover:bg-blue-100 dark:bg-slate-800 dark:hover:bg-slate-700 text-gray-700 dark:text-slate-200 border border-gray-200 dark:border-slate-700 flex items-center space-x-1 transition cursor-pointer text-xs"
          title="Rename item"
        >
          <FbIcon name="rename" size="13px" />
          <span class="hidden sm:inline">Rename</span>
        </button>

        <button
          @click.stop="$emit('delete')"
          class="px-2.5 py-1.5 rounded-xl bg-red-50 dark:bg-red-950/50 hover:bg-red-100 dark:hover:bg-red-900/60 text-red-600 dark:text-red-400 border border-red-200 dark:border-red-800/60 flex items-center space-x-1 transition cursor-pointer text-xs"
          title="Delete selected items"
        >
          <FbIcon name="delete" size="13px" />
          <span class="hidden sm:inline">Delete</span>
        </button>

        <button
          @click.stop="$emit('clear')"
          class="p-1.5 rounded-xl text-gray-400 hover:text-gray-700 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer text-xs font-bold"
          title="Clear Selection (Esc)"
        >
          ✕
        </button>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import FbIcon from '../common/FbIcon.vue';

defineProps<{
  selectedCount: number;
  selectedSize: number;
  singleSelected: boolean;
}>();

defineEmits<{
  (e: 'compress'): void;
  (e: 'rename'): void;
  (e: 'delete'): void;
  (e: 'clear'): void;
}>();

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}
</script>

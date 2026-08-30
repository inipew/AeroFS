<template>
  <Transition name="ios-selection-bar">
    <div
      v-if="selectedCount > 0"
      class="absolute bottom-11 left-1/2 -translate-x-1/2 z-40 bg-white/95 dark:bg-[#0c1220]/95 backdrop-blur-xl border border-gray-200/90 dark:border-slate-700/80 shadow-2xl shadow-blue-500/10 dark:shadow-black/70 ring-1 ring-black/5 dark:ring-white/10 rounded-full px-4 py-2 flex items-center justify-center gap-3 sm:gap-3.5 text-xs font-semibold text-gray-800 dark:text-slate-100 select-none pointer-events-auto transition-all w-max max-w-[calc(100%-32px)] whitespace-nowrap"
    >
      <!-- Left: Indicator & Count -->
      <div class="flex items-center space-x-2 shrink-0">
        <span class="relative flex h-2 w-2">
          <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-blue-400 opacity-75"></span>
          <span class="relative inline-flex rounded-full h-2 w-2 bg-blue-600 dark:bg-blue-500"></span>
        </span>
        <span class="font-bold text-blue-600 dark:text-blue-400 tracking-tight">{{ selectedCount }} selected</span>
        <span v-if="selectedSize > 0" class="text-[11px] font-mono text-gray-500 dark:text-slate-400 font-normal bg-gray-100 dark:bg-slate-800/80 px-2 py-0.5 rounded-full border border-gray-200 dark:border-slate-700/60">
          {{ formatBytes(selectedSize) }}
        </span>
      </div>

      <div class="h-4 w-px bg-gray-200 dark:bg-slate-700/80 shrink-0"></div>

      <!-- Action Buttons -->
      <div class="flex items-center space-x-1.5 shrink-0">
        <button
          @click.stop="$emit('compress')"
          class="px-3 py-1 rounded-full bg-gray-100 hover:bg-gray-200 dark:bg-slate-800 dark:hover:bg-slate-700 text-gray-700 dark:text-slate-200 border border-gray-200/90 dark:border-slate-700/80 flex items-center gap-1.5 transition cursor-pointer text-xs font-medium active:scale-95 shadow-2xs"
          title="Compress selected items"
        >
          <FbIcon name="archive" size="13px" />
          <span>Compress</span>
        </button>

        <button
          v-if="singleSelected"
          @click.stop="$emit('rename')"
          class="px-3 py-1 rounded-full bg-gray-100 hover:bg-gray-200 dark:bg-slate-800 dark:hover:bg-slate-700 text-gray-700 dark:text-slate-200 border border-gray-200/90 dark:border-slate-700/80 flex items-center gap-1.5 transition cursor-pointer text-xs font-medium active:scale-95 shadow-2xs"
          title="Rename item"
        >
          <FbIcon name="rename" size="13px" />
          <span>Rename</span>
        </button>

        <button
          @click.stop="$emit('delete')"
          class="px-3 py-1 rounded-full bg-red-50 hover:bg-red-100 dark:bg-red-950/40 dark:hover:bg-red-900/50 text-red-600 dark:text-red-400 border border-red-200/90 dark:border-red-800/60 flex items-center gap-1.5 transition cursor-pointer text-xs font-medium active:scale-95 shadow-2xs"
          title="Delete selected items"
        >
          <FbIcon name="delete" size="13px" />
          <span>Delete</span>
        </button>

        <!-- Dedicated dismiss button -->
        <button
          @click.stop="$emit('clear')"
          class="w-6 h-6 rounded-full text-gray-400 hover:text-gray-700 dark:hover:text-white hover:bg-gray-200/70 dark:hover:bg-slate-800 border border-transparent hover:border-gray-300 dark:hover:border-slate-700 flex items-center justify-center transition cursor-pointer shrink-0 ml-1 active:scale-90"
          title="Clear Selection (Esc)"
          aria-label="Clear Selection"
        >
          <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <line x1="18" y1="6" x2="6" y2="18"></line>
            <line x1="6" y1="6" x2="18" y2="18"></line>
          </svg>
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

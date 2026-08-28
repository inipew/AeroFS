<template>
  <div class="h-8 border-t border-gray-200/80 dark:border-slate-800/80 px-3.5 sm:px-4 flex items-center justify-between text-[11px] font-medium text-gray-500 dark:text-slate-400 bg-gray-50/80 dark:bg-[#090d18]/90 shrink-0 select-none backdrop-blur-md">
    <div class="flex items-center space-x-2 truncate">
      <span class="font-semibold text-gray-700 dark:text-slate-300">{{ displayedCount }} {{ displayedCount === 1 ? 'item' : 'items' }}</span>
      <span class="text-gray-300 dark:text-slate-700">•</span>
      <span v-if="selectedCount > 0" class="text-blue-600 dark:text-blue-400 font-semibold flex items-center space-x-1">
        <span>{{ selectedCount }} selected</span>
        <span v-if="selectedSize > 0" class="font-mono opacity-80">({{ formatBytes(selectedSize) }})</span>
      </span>
      <span v-else-if="totalFolderSize > 0" class="text-gray-400 dark:text-slate-500 font-mono text-[10px]">
        {{ formatBytes(totalFolderSize) }}
      </span>
    </div>

    <!-- Connection Status & Capabilities Pill -->
    <div class="flex items-center space-x-2 shrink-0 text-[10px] font-mono">
      <span class="hidden sm:inline capitalize text-gray-400 dark:text-slate-500">{{ viewMode }}</span>
      <span class="hidden sm:inline text-gray-300 dark:text-slate-700">•</span>
      <div class="flex items-center space-x-1.5 px-2.5 py-0.5 rounded-full bg-white dark:bg-slate-800/90 border border-gray-200/80 dark:border-slate-700/80 shadow-2xs">
        <span
          :class="[
            'w-1.5 h-1.5 rounded-full shrink-0',
            stale
              ? 'bg-amber-500 animate-pulse'
              : (error ? 'bg-red-500' : 'bg-emerald-500')
          ]"
        ></span>
        <span :class="stale ? 'text-amber-600 dark:text-amber-400 font-semibold' : (error ? 'text-red-500' : 'text-gray-700 dark:text-slate-200 font-medium')">
          {{ stale ? 'Cached' : (error ? 'Error' : currentConnName) }}
        </span>
        <span v-if="isReadOnly" class="text-amber-500 text-[10px]" title="Read-Only">🔒</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  displayedCount: number;
  selectedCount: number;
  selectedSize: number;
  totalFolderSize: number;
  viewMode: string;
  stale?: boolean;
  error: boolean;
  currentConnName: string;
  isReadOnly: boolean;
}>();

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}
</script>

<template>
  <div class="h-8 border-t border-gray-200/80 dark:border-slate-800/80 px-4 flex items-center justify-between text-[11px] font-medium text-gray-500 dark:text-slate-400 bg-gray-50/70 dark:bg-slate-900/50 shrink-0 select-none">
    <div class="flex items-center space-x-2 truncate">
      <span>{{ displayedCount }} items</span>
      <span class="text-gray-300 dark:text-slate-700">•</span>
      <span v-if="selectedCount > 0" class="text-blue-600 dark:text-blue-400 font-semibold">
        {{ selectedCount }} selected
        <span v-if="selectedSize > 0" class="font-mono">({{ formatBytes(selectedSize) }})</span>
      </span>
      <span v-else class="text-gray-400 dark:text-slate-500 font-mono text-[10px]">
        {{ totalFolderSize > 0 ? formatBytes(totalFolderSize) : '' }}
      </span>
    </div>

    <!-- Connection Status & Capabilities Pill -->
    <div class="flex items-center space-x-2.5 shrink-0 text-[10px] font-mono">
      <span class="hidden sm:inline capitalize">{{ viewMode }}</span>
      <span class="hidden sm:inline text-gray-300 dark:text-slate-700">•</span>
      <div class="flex items-center space-x-1.5 px-2 py-0.5 rounded-full bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-700 shadow-2xs">
        <span
          :class="[
            'w-1.5 h-1.5 rounded-full',
            stale
              ? 'bg-amber-500 animate-pulse'
              : (error ? 'bg-red-500' : 'bg-emerald-500')
          ]"
        ></span>
        <span :class="stale ? 'text-amber-600 dark:text-amber-400 font-semibold' : (error ? 'text-red-500' : 'text-emerald-600 dark:text-emerald-400')">
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

<template>
  <nav class="flex items-center space-x-1 text-xs text-slate-300 py-1 px-2 bg-slate-950/40 rounded border border-slate-800 select-none overflow-x-auto">
    <button
      @click="fileStore.navigateTo('/')"
      :class="[
        'px-1.5 py-0.5 rounded hover:bg-slate-800 transition flex items-center',
        fileStore.currentPath === '/' ? 'text-indigo-400 font-semibold' : 'text-slate-400'
      ]"
      title="Root"
    >
      <span>/</span>
    </button>

    <template v-for="(segment, idx) in pathSegments" :key="idx">
      <span class="text-slate-600">/</span>
      <button
        @click="fileStore.navigateTo(segment.path)"
        :class="[
          'px-1.5 py-0.5 rounded hover:bg-slate-800 transition max-w-[150px] truncate',
          idx === pathSegments.length - 1 ? 'text-indigo-300 font-medium bg-indigo-500/10' : 'text-slate-400'
        ]"
      >
        {{ segment.name }}
      </button>
    </template>
  </nav>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useFileStore } from '../../stores/fileStore';

const fileStore = useFileStore();

interface PathSegment {
  name: string;
  path: string;
}

const pathSegments = computed<PathSegment[]>(() => {
  const clean = fileStore.currentPath.split('/').filter(Boolean);
  const segments: PathSegment[] = [];
  let accumulated = '';

  for (const part of clean) {
    accumulated += `/${part}`;
    segments.push({
      name: part,
      path: accumulated,
    });
  }

  return segments;
});
</script>

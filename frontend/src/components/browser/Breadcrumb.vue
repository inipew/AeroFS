<template>
  <nav class="flex items-center space-x-1 text-xs text-slate-300 py-1 px-2 bg-slate-950/40 rounded border border-slate-800 select-none overflow-x-auto">
    <button
      @click="workspaceStore.navigateTo(workspaceStore.activePanelId, '/')"
      :class="[
        'px-1.5 py-0.5 rounded hover:bg-slate-800 transition-[background-color,color,transform] duration-fast ease-spring flex items-center shrink-0 active:scale-95 cursor-pointer',
        currentPath === '/' ? 'text-indigo-400 font-semibold' : 'text-slate-400'
      ]"
      title="Root"
    >
      <span>/</span>
    </button>

    <TransitionGroup name="crumb-item" tag="div" class="flex items-center space-x-1 shrink-0">
      <div v-for="(segment, idx) in pathSegments" :key="segment.path" class="flex items-center space-x-1">
        <span class="text-slate-600">/</span>
        <button
          @click="workspaceStore.navigateTo(workspaceStore.activePanelId, segment.path)"
          :class="[
            'px-1.5 py-0.5 rounded hover:bg-slate-800 transition-[background-color,color,transform] duration-fast ease-spring max-w-[150px] truncate active:scale-95 cursor-pointer',
            idx === pathSegments.length - 1 ? 'text-indigo-300 font-medium bg-indigo-500/10' : 'text-slate-400'
          ]"
        >
          {{ segment.name }}
        </button>
      </div>
    </TransitionGroup>
  </nav>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { getPathSegments } from '../../utils/path';

const workspaceStore = useWorkspaceStore();

const currentPath = computed(() => workspaceStore.activePanel.location.path);

const pathSegments = computed(() => {
  const segments = getPathSegments(currentPath.value);
  // Omit root item since it's rendered as dedicated button
  return segments.filter((s) => s.path !== '/');
});
</script>

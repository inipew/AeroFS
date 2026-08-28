<template>
  <nav class="breadcrumb-capsule flex items-center space-x-1 text-xs select-none overflow-x-auto no-scrollbar shadow-2xs">
    <!-- Root Item -->
    <button
      @click="workspaceStore.navigateTo(targetPanelId, '/')"
      :class="[
        'px-2 py-0.5 rounded-lg transition-[background-color,color,transform] duration-fast ease-spring flex items-center space-x-1.5 shrink-0 active:scale-95 cursor-pointer font-medium',
        currentPath === '/'
          ? 'text-blue-600 dark:text-blue-400 font-semibold bg-blue-50/80 dark:bg-blue-950/40'
          : 'text-gray-600 dark:text-slate-400 hover:text-gray-900 dark:hover:text-white hover:bg-gray-200/60 dark:hover:bg-slate-700/60'
      ]"
      title="Root (/)"
    >
      <FbIcon :name="connectionProvider === 'local' ? 'folder' : 'share'" size="13px" class="text-blue-500 shrink-0" />
      <span class="truncate max-w-[100px] sm:max-w-[140px]">{{ connectionName }}</span>
    </button>

    <!-- Path Segments Transition Group -->
    <TransitionGroup name="crumb-item" tag="div" class="flex items-center space-x-1 shrink-0">
      <div v-for="(segment, idx) in pathSegments" :key="segment.path" class="flex items-center space-x-1">
        <span class="text-gray-400 dark:text-slate-600 text-xs font-bold shrink-0 select-none">›</span>
        <button
          @click="workspaceStore.navigateTo(targetPanelId, segment.path)"
          :class="[
            'px-2 py-0.5 rounded-lg transition-[background-color,color,transform] duration-fast ease-spring max-w-[140px] sm:max-w-[180px] truncate active:scale-95 cursor-pointer',
            idx === pathSegments.length - 1
              ? 'text-gray-900 dark:text-white font-bold bg-gray-200/70 dark:bg-slate-700/60'
              : 'text-gray-600 dark:text-slate-400 hover:text-gray-900 dark:hover:text-white hover:bg-gray-200/60 dark:hover:bg-slate-700/60 font-medium'
          ]"
          :title="segment.name"
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
import { useConnectionStore } from '../../stores/connectionStore';
import { getPathSegments } from '../../utils/path';
import FbIcon from '../common/FbIcon.vue';
import type { PanelId } from '../../types/workspace';

const props = defineProps<{
  panelId?: PanelId;
}>();

const workspaceStore = useWorkspaceStore();
const connStore = useConnectionStore();

const targetPanelId = computed<PanelId>(() => props.panelId || workspaceStore.activePanelId);
const currentPanel = computed(() => workspaceStore.getPanel(targetPanelId.value));
const currentPath = computed(() => currentPanel.value.location.path);

const currentConnection = computed(() =>
  connStore.connections.find((c) => c.id === currentPanel.value.location.connectionId)
);

const connectionProvider = computed(() => currentConnection.value?.provider || 'local');
const connectionName = computed(() => currentConnection.value?.name || 'Local Storage');

const pathSegments = computed(() => {
  const segments = getPathSegments(currentPath.value);
  // Omit root item since it's rendered as dedicated button
  return segments.filter((s) => s.path !== '/');
});
</script>

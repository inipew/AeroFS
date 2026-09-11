<template>
  <main
    ref="mainContainerRef"
    :style="{ '--split-ratio': workspaceStore.splitRatio }"
    class="flex-1 flex overflow-hidden min-w-0 bg-white dark:bg-[#0b0f19] p-0 relative"
  >
    <!-- MOBILE VIEW: Continuous Dual-Slide Track with Touch Gestures -->
    <div
      v-if="uiStore.isMobile"
      ref="mobileTrackWrapperRef"
      class="w-full h-full flex flex-col min-w-0 overflow-hidden relative touch-pan-y"
      @touchstart="swipe.onTouchStart"
      @touchmove="swipe.onTouchMove"
      @touchend="swipe.onTouchEnd"
      @touchcancel="swipe.onTouchCancel"
    >
      <!-- Dual Slide Container when Dual-Pane is enabled -->
      <div
        v-if="workspaceStore.isDualPane"
        class="w-[200%] h-full flex flex-row flex-nowrap will-change-transform"
        :style="swipe.trackStyle.value"
      >
        <div class="w-1/2 h-full flex flex-col min-w-0 overflow-hidden shrink-0">
          <FilePane panelId="left" />
        </div>
        <div class="w-1/2 h-full flex flex-col min-w-0 overflow-hidden shrink-0">
          <FilePane panelId="right" />
        </div>
      </div>

      <!-- Single Panel View on Mobile when Dual-Pane is disabled -->
      <div v-else class="w-full h-full flex flex-col min-w-0 overflow-hidden">
        <FilePane panelId="left" />
      </div>
    </div>

    <!-- DESKTOP VIEW: Continuous High-Performance Split Pane -->
    <div v-else class="w-full h-full flex flex-row min-w-0 overflow-hidden">
      <!-- Single Pane Layout on Desktop -->
      <div v-if="!workspaceStore.isDualPane" class="w-full h-full flex flex-col min-w-0 overflow-hidden">
        <FilePane panelId="left" />
      </div>

      <!-- Dual Pane Split Layout on Desktop -->
      <template v-else>
        <!-- Left Panel Container -->
        <div
          :style="{
            width: 'calc(var(--split-ratio) * 100% - 3px)'
          }"
          class="h-full flex flex-col min-w-[200px]"
        >
          <FilePane panelId="left" />
        </div>

        <!-- Resizable Splitter Divider Handle -->
        <WorkspaceSplitter
          :is-dragging="splitPane.isDragging.value"
          @pointerdown="splitPane.onPointerDown"
        />

        <!-- Right Panel Container -->
        <div
          :style="{
            width: 'calc((1 - var(--split-ratio)) * 100% - 3px)'
          }"
          class="h-full flex flex-col min-w-[200px]"
        >
          <FilePane panelId="right" />
        </div>
      </template>
    </div>
  </main>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import { useWorkspaceStore } from '../stores/workspaceStore';
import { useUiStore } from '../stores/uiStore';
import { useSplitPane } from './useSplitPane';
import { usePaneSwipe } from './usePaneSwipe';
import WorkspaceSplitter from './WorkspaceSplitter.vue';
import FilePane from '../features/files/FilePane.vue';

const workspaceStore = useWorkspaceStore();
const uiStore = useUiStore();

const mainContainerRef = ref<HTMLElement | null>(null);
const mobileTrackWrapperRef = ref<HTMLElement | null>(null);

// Desktop Split Pane composable
const splitPane = useSplitPane(mainContainerRef, {
  initialRatio: workspaceStore.splitRatio,
  min: 0.18,
  max: 0.82,
  onChange: (ratio) => {
    workspaceStore.setSplitRatio(ratio);
  },
});

// Mobile Gesture Swipe composable
const activePaneRef = computed<'left' | 'right'>({
  get: () => workspaceStore.activePanelId,
  set: (val) => workspaceStore.setActivePanel(val),
});

const isSwipeEnabled = computed(() => uiStore.isMobile && workspaceStore.isDualPane);

const swipe = usePaneSwipe({
  activePane: activePaneRef,
  enabled: isSwipeEnabled,
  containerRef: mobileTrackWrapperRef,
  onChange: (targetPane) => {
    workspaceStore.setActivePanel(targetPane);
  },
});
</script>

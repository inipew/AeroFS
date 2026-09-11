<template>
  <div
    class="w-full h-full flex items-center justify-center relative overflow-hidden select-none touch-none"
    :class="[
      transform.zoomLevel.value > 1
        ? transform.isPanning.value
          ? 'cursor-grabbing'
          : 'cursor-grab'
        : 'cursor-default'
    ]"
    @wheel.prevent="transform.handleWheel"
    @dblclick.prevent="transform.handleDoubleClick"
    @mousedown="transform.startPan"
    @mousemove="transform.doPan"
    @mouseup="transform.stopPan"
    @mouseleave="transform.stopPan"
    @touchstart="transform.startPan"
    @touchmove="transform.doPan"
    @touchend="transform.stopPan"
    @touchcancel="transform.stopPan"
  >
    <!-- Image Display Element -->
    <img
      :src="store.activeUrl"
      :alt="store.activeTitle"
      @load="handleImageLoad"
      @error="store.setPlaybackError(true)"
      :style="transform.transformStyle.value"
      class="max-w-[92vw] sm:max-w-[90vw] max-h-[85vh] object-contain rounded-lg shadow-2xl pointer-events-none select-none transition-shadow"
      draggable="false"
    />

    <!-- Floating Image Controls at Bottom -->
    <div
      v-if="!store.isFilmstripOpen"
      class="absolute bottom-6 inset-x-0 z-30 flex justify-center pointer-events-none"
    >
      <ImageControls
        :zoom-level="transform.zoomLevel.value"
        @zoom-in="transform.zoomIn()"
        @zoom-out="transform.zoomOut()"
        @reset-zoom="transform.resetZoom()"
        @set-zoom="(val) => transform.setZoom(val)"
        @rotate="transform.rotateClockwise()"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { watch } from 'vue';
import ImageControls from './ImageControls.vue';
import { useMediaViewerStore } from '../../stores/mediaViewerStore';
import { useImageTransform } from '../../composables/useImageTransform';

const store = useMediaViewerStore();
const transform = useImageTransform();

function handleImageLoad(e: Event) {
  const target = e.target as HTMLImageElement | null;
  if (target && target.naturalWidth && target.naturalHeight) {
    store.setMediaDimensions(target.naturalWidth, target.naturalHeight);
  }
}

watch(
  () => store.activeUrl,
  () => {
    transform.resetTransform();
  }
);

defineExpose({
  transform,
});
</script>

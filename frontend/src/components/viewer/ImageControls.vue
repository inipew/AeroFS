<template>
  <div
    class="bg-black/60 backdrop-blur-xl border border-white/10 rounded-2xl px-2 py-1.5 shadow-2xl flex items-center space-x-1 sm:space-x-1.5 text-xs text-white/80 select-none pointer-events-auto"
  >
    <!-- Zoom Out -->
    <button
      type="button"
      @click="$emit('zoomOut')"
      class="p-1.5 rounded-xl hover:bg-white/10 hover:text-white transition cursor-pointer"
      title="Zoom Out (-)"
    >
      <FbIcon name="minus" size="14px" />
    </button>

    <!-- Percentage Indicator / Reset Zoom -->
    <button
      type="button"
      @click="$emit('resetZoom')"
      class="px-2 py-1 rounded-lg hover:bg-white/10 hover:text-white font-mono text-[11px] font-semibold transition cursor-pointer min-w-[48px] text-center"
      title="Reset to 100% (0)"
    >
      {{ Math.round(zoomLevel * 100) }}%
    </button>

    <!-- Zoom In -->
    <button
      type="button"
      @click="$emit('zoomIn')"
      class="p-1.5 rounded-xl hover:bg-white/10 hover:text-white transition cursor-pointer"
      title="Zoom In (+)"
    >
      <FbIcon name="plus" size="14px" />
    </button>

    <!-- Divider -->
    <div class="h-4 w-[1px] bg-white/10 mx-0.5 sm:mx-1"></div>

    <!-- Fit to Screen -->
    <button
      type="button"
      @click="$emit('resetZoom')"
      class="px-2 py-1 rounded-lg hover:bg-white/10 hover:text-white font-medium text-[11px] transition cursor-pointer"
      title="Fit to Screen (0)"
    >
      Fit
    </button>

    <!-- 1:1 Pixel Mapping -->
    <button
      type="button"
      @click="$emit('setZoom', 1.0)"
      class="px-2 py-1 rounded-lg hover:bg-white/10 hover:text-white font-mono text-[11px] transition cursor-pointer hidden sm:inline-block"
      title="Actual Size (1:1)"
    >
      1:1
    </button>

    <!-- Rotate Clockwise 90° -->
    <button
      type="button"
      @click="$emit('rotate')"
      class="p-1.5 rounded-xl hover:bg-white/10 hover:text-white transition cursor-pointer"
      title="Rotate 90° (R)"
    >
      <FbIcon name="refresh" size="14px" />
    </button>
  </div>
</template>

<script setup lang="ts">
import FbIcon from '../common/FbIcon.vue';

defineProps<{
  zoomLevel: number;
}>();

defineEmits<{
  (e: 'zoomIn'): void;
  (e: 'zoomOut'): void;
  (e: 'resetZoom'): void;
  (e: 'setZoom', val: number): void;
  (e: 'rotate'): void;
}>();
</script>

<template>
  <div v-if="store.hasMultiple" class="contents select-none">
    <!-- Left Edge Control (Previous) -->
    <button
      type="button"
      @click.stop="store.navigate('prev')"
      class="absolute left-2 sm:left-4 top-1/2 -translate-y-1/2 z-40 w-14 sm:w-16 h-24 rounded-2xl flex items-center justify-center text-white/50 hover:text-white bg-black/20 hover:bg-black/60 backdrop-blur-xs border border-transparent hover:border-white/10 opacity-35 hover:opacity-100 transition-all duration-200 cursor-pointer group"
      title="Previous (← Arrow)"
    >
      <FbIcon
        name="chevron-left"
        size="28px"
        class="group-hover:-translate-x-1 transition-transform duration-150"
      />
    </button>

    <!-- Right Edge Control (Next) -->
    <button
      type="button"
      @click.stop="store.navigate('next')"
      class="absolute right-2 sm:right-4 top-1/2 -translate-y-1/2 z-40 w-14 sm:w-16 h-24 rounded-2xl flex items-center justify-center text-white/50 hover:text-white bg-black/20 hover:bg-black/60 backdrop-blur-xs border border-transparent hover:border-white/10 opacity-35 hover:opacity-100 transition-all duration-200 cursor-pointer group"
      title="Next (→ Arrow)"
    >
      <FbIcon
        name="chevron-right"
        size="28px"
        class="group-hover:translate-x-1 transition-transform duration-150"
      />
    </button>
  </div>
</template>

<script setup lang="ts">
import FbIcon from '../common/FbIcon.vue';
import { useMediaViewerStore } from '../../stores/mediaViewerStore';

const store = useMediaViewerStore();
</script>

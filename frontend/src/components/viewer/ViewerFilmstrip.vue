<template>
  <div v-if="store.hasMultiple" class="contents select-none">
    <!-- Persistent Transparent Bottom Hover Sensor Zone -->
    <div
      class="absolute bottom-0 inset-x-0 h-16 z-30 pointer-events-auto"
      @mouseenter="isHovered = true"
    ></div>

    <!-- Floating Filmstrip Bar -->
    <Transition name="fade-slide-up">
      <div
        v-if="store.isFilmstripOpen || isHovered"
        class="absolute bottom-4 sm:bottom-6 inset-x-0 z-40 flex flex-col items-center pointer-events-none"
        @mouseenter="isHovered = true"
        @mouseleave="isHovered = false"
      >
        <div
          class="bg-black/75 backdrop-blur-xl border border-white/10 rounded-2xl p-2 shadow-2xl flex flex-col items-center pointer-events-auto max-w-[94vw] sm:max-w-xl md:max-w-2xl"
        >
          <!-- Thumbnail Row -->
          <div
            class="flex items-center space-x-2 overflow-x-auto py-1 px-1 max-w-full scrollbar-none"
          >
            <button
              v-for="(item, idx) in store.playlist"
              :key="item.path"
              type="button"
              :ref="(el) => { if (idx === store.currentIndex) activeItemRef = el as HTMLElement; }"
              @click="store.selectEntry(item)"
              class="relative w-11 h-11 sm:w-12 sm:h-12 rounded-xl overflow-hidden shrink-0 border transition-all duration-150 cursor-pointer group flex items-center justify-center bg-white/5"
              :class="[
                idx === store.currentIndex
                  ? 'ring-2 ring-blue-500 border-blue-400 scale-105 shadow-md shadow-blue-500/20'
                  : 'border-white/10 hover:border-white/30 opacity-60 hover:opacity-100'
              ]"
              :title="item.name"
            >
              <!-- Image Thumbnail -->
              <img
                v-if="detectMediaKind(item) === 'image'"
                :src="getContentUrl(store.connectionId, item.path)"
                :alt="item.name"
                loading="lazy"
                class="w-full h-full object-cover pointer-events-none"
              />

              <!-- Video Thumbnail Fallback -->
              <div
                v-else-if="detectMediaKind(item) === 'video'"
                class="w-full h-full flex items-center justify-center bg-purple-950/40 text-purple-400"
              >
                <FbIcon name="video" size="18px" />
              </div>

              <!-- Audio Thumbnail Fallback -->
              <div
                v-else-if="detectMediaKind(item) === 'audio'"
                class="w-full h-full flex items-center justify-center bg-emerald-950/40 text-emerald-400"
              >
                <FbIcon name="music" size="18px" />
              </div>

              <!-- Generic File Fallback -->
              <div v-else class="w-full h-full flex items-center justify-center text-white/50">
                <FbIcon name="file" size="16px" />
              </div>
            </button>
          </div>

          <!-- Position Counter Indicator -->
          <div class="text-[10px] font-mono text-white/50 mt-1 select-none flex items-center space-x-1">
            <span class="font-bold text-white/80">{{ store.currentIndex + 1 }}</span>
            <span>/</span>
            <span>{{ store.playlist.length }}</span>
          </div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, nextTick } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { useMediaViewerStore, detectMediaKind } from '../../stores/mediaViewerStore';
import { getContentUrl } from '../../api/files';

const store = useMediaViewerStore();
const isHovered = ref(false);

const activeItemRef = ref<HTMLElement | null>(null);

watch(
  () => store.currentIndex,
  () => {
    void nextTick(() => {
      activeItemRef.value?.scrollIntoView({
        behavior: 'smooth',
        block: 'nearest',
        inline: 'center',
      });
    });
  }
);
</script>

<style scoped>
.scrollbar-none::-webkit-scrollbar {
  display: none;
}
.scrollbar-none {
  -ms-overflow-style: none;
  scrollbar-width: none;
}

.fade-slide-up-enter-active,
.fade-slide-up-leave-active {
  transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

.fade-slide-up-enter-from,
.fade-slide-up-leave-to {
  opacity: 0;
  transform: translateY(12px);
}
</style>

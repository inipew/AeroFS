<template>
  <Transition name="fade-sheet">
    <div
      v-if="store.isMobileMenuOpen"
      class="fixed inset-0 z-50 bg-black/60 backdrop-blur-xs flex flex-col justify-end p-3 sm:hidden select-none"
      @click="store.toggleMobileMenu(false)"
    >
      <div
        class="bg-[#161a22] border border-white/10 rounded-2xl p-4 shadow-2xl space-y-2 text-xs text-white"
        @click.stop
      >
        <!-- Header / Pill Handle -->
        <div class="flex flex-col items-center pb-2">
          <div class="w-10 h-1 bg-white/20 rounded-full mb-3"></div>
          <span class="font-bold text-sm truncate max-w-[280px]">{{ store.activeTitle }}</span>
        </div>

        <div class="space-y-1">
          <!-- File Information -->
          <button
            type="button"
            @click="store.toggleInfo(true); store.toggleMobileMenu(false);"
            class="w-full py-2.5 px-3 rounded-xl hover:bg-white/10 flex items-center space-x-3 transition cursor-pointer"
          >
            <FbIcon name="info" size="16px" class="text-blue-400" />
            <span class="font-medium">File Information</span>
          </button>

          <!-- Filmstrip -->
          <button
            v-if="store.hasMultiple"
            type="button"
            @click="store.toggleFilmstrip(); store.toggleMobileMenu(false);"
            class="w-full py-2.5 px-3 rounded-xl hover:bg-white/10 flex items-center space-x-3 transition cursor-pointer"
          >
            <FbIcon name="layers" size="16px" class="text-amber-400" />
            <span class="font-medium">Toggle Filmstrip</span>
          </button>

          <!-- Download -->
          <a
            :href="store.downloadUrl"
            :download="store.activeTitle"
            class="w-full py-2.5 px-3 rounded-xl hover:bg-white/10 flex items-center space-x-3 transition cursor-pointer"
          >
            <FbIcon name="download" size="16px" class="text-emerald-400" />
            <span class="font-medium">Download File</span>
          </a>

          <!-- Rotate Image (If Image) -->
          <button
            v-if="store.activeMediaKind === 'image'"
            type="button"
            @click="$emit('rotateImage'); store.toggleMobileMenu(false);"
            class="w-full py-2.5 px-3 rounded-xl hover:bg-white/10 flex items-center space-x-3 transition cursor-pointer"
          >
            <FbIcon name="refresh" size="16px" class="text-purple-400" />
            <span class="font-medium">Rotate 90°</span>
          </button>

          <!-- Fullscreen -->
          <button
            type="button"
            @click="$emit('toggleFullscreen'); store.toggleMobileMenu(false);"
            class="w-full py-2.5 px-3 rounded-xl hover:bg-white/10 flex items-center space-x-3 transition cursor-pointer"
          >
            <FbIcon name="maximize" size="16px" class="text-cyan-400" />
            <span class="font-medium">Toggle Fullscreen</span>
          </button>
        </div>

        <div class="pt-2 border-t border-white/[0.08]">
          <button
            type="button"
            @click="store.toggleMobileMenu(false)"
            class="w-full py-2.5 rounded-xl bg-white/10 hover:bg-white/15 text-white/80 font-semibold transition cursor-pointer text-center"
          >
            Cancel
          </button>
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import FbIcon from '../common/FbIcon.vue';
import { useMediaViewerStore } from '../../stores/mediaViewerStore';

const store = useMediaViewerStore();

defineEmits<{
  (e: 'rotateImage'): void;
  (e: 'toggleFullscreen'): void;
}>();
</script>

<style scoped>
.fade-sheet-enter-active,
.fade-sheet-leave-active {
  transition: all 0.25s cubic-bezier(0.16, 1, 0.3, 1);
}

.fade-sheet-enter-from,
.fade-sheet-leave-to {
  opacity: 0;
  transform: translateY(100%);
}
</style>

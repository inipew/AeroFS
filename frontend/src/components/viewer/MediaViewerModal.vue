<template>
  <Transition name="fade-backdrop">
    <div
      v-if="store.isOpen"
      ref="containerRef"
      class="fixed inset-0 z-50 bg-[#08090b] flex flex-col select-none font-sans text-xs overflow-hidden"
      @touchstart.passive="handleTouchStart"
      @touchend="handleTouchEnd"
    >
      <!-- Top Floating Header -->
      <ViewerHeader
        :is-fullscreen="isFullscreen"
        @toggle-fullscreen="toggleFullscreen"
      />

      <!-- Main Canvas Viewport -->
      <main class="flex-1 min-h-0 relative flex items-center justify-center overflow-hidden">
        <!-- 0. Playback Error Fallback (Codec / Load Failure) -->
        <div
          v-if="store.playbackError"
          class="text-center text-white/70 space-y-4 max-w-md p-8 bg-[#12151b]/90 border border-white/10 rounded-3xl backdrop-blur-xl shadow-2xl animate-in zoom-in-95 duration-150 select-none z-20"
        >
          <div class="w-16 h-16 rounded-2xl bg-amber-500/10 text-amber-400 flex items-center justify-center text-2xl mx-auto border border-amber-500/20">
            <FbIcon name="alert-triangle" size="28px" />
          </div>
          <div>
            <h3 class="text-base font-bold text-white mb-1">Playback Unsupported in Browser</h3>
            <p class="text-xs text-white/60 leading-relaxed">
              This media format or codec cannot be decoded natively by your browser. You can download the file to open it in an external media player.
            </p>
          </div>
          <a
            :href="store.downloadUrl"
            :download="store.activeTitle"
            class="px-5 py-2.5 bg-blue-600 hover:bg-blue-500 text-white rounded-xl font-bold inline-flex items-center space-x-2 shadow-lg transition cursor-pointer"
          >
            <FbIcon name="download" size="16px" />
            <span>Download {{ store.activeTitle }}</span>
          </a>
        </div>

        <!-- 1. Image Viewer -->
        <ImageViewer
          v-else-if="store.activeMediaKind === 'image'"
          ref="imageViewerRef"
        />

        <!-- 2. Video Player -->
        <VideoViewer
          v-else-if="store.activeMediaKind === 'video'"
          ref="videoViewerRef"
        />

        <!-- 3. Audio Player -->
        <AudioViewer
          v-else-if="store.activeMediaKind === 'audio'"
          ref="audioViewerRef"
        />

        <!-- 4. Fallback / Other Format -->
        <div
          v-else
          class="text-center text-white/60 space-y-4 max-w-sm p-8 bg-[#12151b]/90 border border-white/10 rounded-3xl backdrop-blur-xl shadow-2xl z-20"
        >
          <div class="w-16 h-16 rounded-2xl bg-white/5 text-white/50 flex items-center justify-center mx-auto border border-white/10">
            <FbIcon name="file" size="28px" />
          </div>
          <div>
            <h3 class="text-base font-bold text-white mb-1">Preview not available</h3>
            <p class="text-xs text-white/50">You can download this file to view it on your device.</p>
          </div>
          <a
            :href="store.downloadUrl"
            :download="store.activeTitle"
            class="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-xl font-medium inline-flex items-center space-x-2 shadow transition cursor-pointer"
          >
            <FbIcon name="download" size="14px" />
            <span>Download File</span>
          </a>
        </div>

        <!-- Floating Edge Navigation (‹ and ›) -->
        <ViewerNavigation />

        <!-- Collapsible Bottom Filmstrip -->
        <ViewerFilmstrip />

        <!-- Right / Bottom Metadata Info Drawer -->
        <ViewerInfoPanel />

        <!-- Mobile Action Sheet (⋯) -->
        <ViewerMobileMenu
          @rotate-image="handleRotateImage"
          @toggle-fullscreen="toggleFullscreen"
        />
      </main>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { useMediaViewerStore } from '../../stores/mediaViewerStore';
import { useViewerKeyboard } from '../../composables/useViewerKeyboard';

import ViewerHeader from './ViewerHeader.vue';
import ViewerNavigation from './ViewerNavigation.vue';
import ViewerFilmstrip from './ViewerFilmstrip.vue';
import ViewerInfoPanel from './ViewerInfoPanel.vue';
import ViewerMobileMenu from './ViewerMobileMenu.vue';

import ImageViewer from './ImageViewer.vue';
import VideoViewer from './VideoViewer.vue';
import AudioViewer from './AudioViewer.vue';

const store = useMediaViewerStore();

const containerRef = ref<HTMLElement | null>(null);
const imageViewerRef = ref<InstanceType<typeof ImageViewer> | null>(null);
const videoViewerRef = ref<InstanceType<typeof VideoViewer> | null>(null);
const audioViewerRef = ref<InstanceType<typeof AudioViewer> | null>(null);

const isFullscreen = ref(false);

// Mobile horizontal swipe gesture detection
let touchStartX = 0;
let touchStartY = 0;

function handleTouchStart(e: TouchEvent) {
  if (e.touches.length === 1) {
    touchStartX = e.touches[0].clientX;
    touchStartY = e.touches[0].clientY;
  }
}

function handleTouchEnd(e: TouchEvent) {
  if (e.changedTouches.length !== 1) return;
  // Guard: Do not trigger horizontal swipe if image is currently zoomed in (> 1.05x)
  if (
    store.activeMediaKind === 'image' &&
    imageViewerRef.value?.transform &&
    imageViewerRef.value.transform.zoomLevel.value > 1.05
  ) {
    return;
  }

  const deltaX = e.changedTouches[0].clientX - touchStartX;
  const deltaY = e.changedTouches[0].clientY - touchStartY;

  // Ensure gesture is predominantly horizontal (at least 60px distance, > 1.5x vertical delta)
  if (Math.abs(deltaX) > 60 && Math.abs(deltaX) > Math.abs(deltaY) * 1.5) {
    if (deltaX < 0) {
      store.navigate('next');
    } else {
      store.navigate('prev');
    }
  }
}

function handleFullscreenChange() {
  isFullscreen.value = !!document.fullscreenElement;
}

function toggleFullscreen() {
  if (!document.fullscreenElement) {
    void containerRef.value?.requestFullscreen?.();
    isFullscreen.value = true;
  } else {
    void document.exitFullscreen?.();
    isFullscreen.value = false;
  }
}

function handleRotateImage() {
  imageViewerRef.value?.transform.rotateClockwise();
}

function handleTogglePlay() {
  if (store.activeMediaKind === 'video') {
    videoViewerRef.value?.togglePlay();
  } else if (store.activeMediaKind === 'audio') {
    audioViewerRef.value?.togglePlay();
  }
}

// Global Viewer Keyboard Shortcuts
useViewerKeyboard({
  onClose: () => store.close(),
  onPrev: () => store.navigate('prev'),
  onNext: () => store.navigate('next'),
  onTogglePlay: handleTogglePlay,
  onZoomIn: () => imageViewerRef.value?.transform.zoomIn(),
  onZoomOut: () => imageViewerRef.value?.transform.zoomOut(),
  onResetZoom: () => imageViewerRef.value?.transform.resetZoom(),
  onRotate: () => imageViewerRef.value?.transform.rotateClockwise(),
  onToggleFullscreen: toggleFullscreen,
});

onMounted(() => {
  document.addEventListener('fullscreenchange', handleFullscreenChange);
});

onUnmounted(() => {
  document.removeEventListener('fullscreenchange', handleFullscreenChange);
  if (document.fullscreenElement) {
    void document.exitFullscreen?.();
  }
});
</script>

<style scoped>
.fade-backdrop-enter-active,
.fade-backdrop-leave-active {
  transition: opacity 0.2s ease-out;
}

.fade-backdrop-enter-from,
.fade-backdrop-leave-to {
  opacity: 0;
}
</style>

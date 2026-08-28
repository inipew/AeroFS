<template>
  <Transition name="ios-modal">
    <div
      v-if="uiStore.isMediaViewerOpen"
      class="fixed inset-0 z-50 bg-black/90 backdrop-blur-md flex flex-col p-2 sm:p-6 select-none font-sans text-xs"
      tabindex="0"
      @keydown="handleKeyDown"
    >
      <!-- Modal Header Toolbar -->
      <div class="modal-card h-14 bg-slate-900/95 border border-slate-700/90 px-4 rounded-t-2xl flex items-center justify-between text-slate-200 shadow-xl shrink-0">
      <!-- Left: Media Title, Type Icon & Playlist Index -->
      <div class="flex items-center space-x-3 truncate">
        <span class="text-lg shrink-0">{{ getMediaTypeIcon() }}</span>
        <div class="truncate flex items-center space-x-2">
          <span class="font-bold text-white text-sm truncate max-w-xs sm:max-w-md" :title="uiStore.mediaViewerTitle">
            {{ uiStore.mediaViewerTitle }}
          </span>
          <span
            v-if="uiStore.mediaViewerList.length > 1"
            class="text-[10px] font-mono px-2 py-0.5 rounded-full bg-slate-800 text-slate-400 border border-slate-700 shrink-0"
          >
            {{ currentIndex + 1 }} / {{ uiStore.mediaViewerList.length }}
          </span>
        </div>
      </div>

      <!-- Right: Action Controls (Zoom, Rotate, Speed, Fullscreen, Download, Close) -->
      <div class="flex items-center space-x-1.5 shrink-0">
        <!-- Image Specific Controls -->
        <template v-if="mediaType === 'image'">
          <button
            @click="zoomIn"
            class="p-2 text-slate-400 hover:text-white hover:bg-slate-800 rounded-xl transition cursor-pointer"
            title="Zoom In (+)"
          >
            <FbIcon name="plus" size="15px" />
          </button>

          <button
            @click="zoomOut"
            class="p-2 text-slate-400 hover:text-white hover:bg-slate-800 rounded-xl transition cursor-pointer"
            title="Zoom Out (-)"
          >
            <FbIcon name="minus" size="15px" />
          </button>

          <button
            @click="resetZoom"
            class="px-2 py-1 text-[11px] font-mono text-slate-300 hover:text-white hover:bg-slate-800 rounded-lg transition cursor-pointer"
            title="Reset Zoom"
          >
            {{ Math.round(zoomLevel * 100) }}%
          </button>

          <button
            @click="rotateClockwise"
            class="p-2 text-slate-400 hover:text-white hover:bg-slate-800 rounded-xl transition cursor-pointer"
            title="Rotate 90° Clockwise (R)"
          >
            <FbIcon name="refresh" size="15px" />
          </button>
        </template>

        <!-- Video Specific Controls -->
        <template v-if="mediaType === 'video'">
          <!-- Playback Speed Menu -->
          <div class="relative">
            <button
              @click="isSpeedMenuOpen = !isSpeedMenuOpen"
              class="px-2 py-1 text-[11px] font-mono text-slate-300 hover:text-white hover:bg-slate-800 rounded-lg transition cursor-pointer flex items-center space-x-1"
              title="Playback Speed"
            >
              <span>{{ playbackSpeed }}x</span>
            </button>

            <div
              v-if="isSpeedMenuOpen"
              @click="isSpeedMenuOpen = false"
              class="absolute right-0 mt-2 w-24 bg-slate-900 border border-slate-700 rounded-xl shadow-2xl p-1 z-50 text-xs space-y-0.5"
            >
              <button
                v-for="spd in [0.5, 0.75, 1.0, 1.25, 1.5, 2.0]"
                :key="spd"
                @click="setPlaybackSpeed(spd)"
                :class="[
                  'w-full text-left px-2.5 py-1.5 rounded-lg transition font-mono',
                  playbackSpeed === spd ? 'bg-blue-600 text-white font-bold' : 'text-slate-300 hover:bg-slate-800'
                ]"
              >
                {{ spd }}x
              </button>
            </div>
          </div>

          <!-- PiP Button -->
          <button
            @click="togglePiP"
            class="p-2 text-slate-400 hover:text-white hover:bg-slate-800 rounded-xl transition cursor-pointer"
            title="Picture-in-Picture"
          >
            <FbIcon name="panel-right" size="15px" />
          </button>
        </template>

        <!-- Fullscreen Button -->
        <button
          @click="toggleFullscreen"
          class="p-2 text-slate-400 hover:text-white hover:bg-slate-800 rounded-xl transition cursor-pointer"
          title="Toggle Fullscreen (F)"
        >
          <FbIcon name="maximize" size="15px" />
        </button>

        <div class="h-4 w-[1px] bg-slate-700 mx-1"></div>

        <!-- Download Button -->
        <a
          :href="uiStore.mediaViewerUrl"
          download
          class="px-3 py-1.5 bg-blue-600 hover:bg-blue-700 text-white font-semibold rounded-xl text-xs transition shadow-xs flex items-center space-x-1.5"
          title="Download File"
        >
          <FbIcon name="download" size="14px" />
          <span>Download</span>
        </a>

        <!-- Close Button -->
        <button
          @click="closeViewer"
          class="p-2 text-slate-400 hover:text-white hover:bg-red-500/20 hover:text-red-400 rounded-xl text-sm transition cursor-pointer ml-1"
          title="Close (Esc)"
        >
          ✕
        </button>
      </div>
    </div>

    <!-- Main Media Display Viewport -->
    <div
      ref="viewportRef"
      class="flex-1 bg-slate-950/95 border-x border-b border-slate-700/90 rounded-b-2xl flex items-center justify-center relative overflow-hidden p-4 select-none"
      @mousedown="startPan"
      @mousemove="doPan"
      @mouseup="stopPan"
      @mouseleave="stopPan"
    >
      <!-- Floating Navigation: Previous Media Button -->
      <button
        v-if="uiStore.mediaViewerList.length > 1"
        @click.stop="uiStore.navigateMedia('prev'); resetImageTransform()"
        class="absolute left-4 z-40 w-11 h-11 rounded-full bg-slate-900/80 hover:bg-blue-600 text-white backdrop-blur-md border border-slate-700 hover:border-blue-500 flex items-center justify-center transition shadow-2xl cursor-pointer group"
        title="Previous (← Arrow)"
      >
        <FbIcon name="chevron-left" size="20px" class="group-hover:-translate-x-0.5 transition transform" />
      </button>

      <!-- Floating Navigation: Next Media Button -->
      <button
        v-if="uiStore.mediaViewerList.length > 1"
        @click.stop="uiStore.navigateMedia('next'); resetImageTransform()"
        class="absolute right-4 z-40 w-11 h-11 rounded-full bg-slate-900/80 hover:bg-blue-600 text-white backdrop-blur-md border border-slate-700 hover:border-blue-500 flex items-center justify-center transition shadow-2xl cursor-pointer group"
        title="Next (→ Arrow)"
      >
        <FbIcon name="chevron-right" size="20px" class="group-hover:translate-x-0.5 transition transform" />
      </button>

      <!-- 0. ERROR FALLBACK (Unsupported Codec / Load Failure) -->
      <div
        v-if="playbackError"
        class="text-center text-slate-400 space-y-4 max-w-md p-8 bg-slate-900/90 border border-slate-800 rounded-3xl backdrop-blur-md shadow-2xl animate-in zoom-in-95 duration-150"
      >
        <div class="w-16 h-16 rounded-2xl bg-amber-500/10 text-amber-400 flex items-center justify-center text-2xl mx-auto border border-amber-500/20">
          ⚠️
        </div>
        <div>
          <h3 class="text-base font-bold text-white mb-1">Playback Unsupported in Browser</h3>
          <p class="text-xs text-slate-400 leading-relaxed">
            This media codec or format cannot be decoded natively by your browser. You can download the file to open it in a local media player (e.g. VLC).
          </p>
        </div>
        <a
          :href="uiStore.mediaViewerUrl"
          download
          class="px-5 py-2.5 bg-blue-600 hover:bg-blue-700 active:bg-blue-800 text-white rounded-xl font-bold inline-flex items-center space-x-2 shadow-lg transition cursor-pointer"
        >
          <FbIcon name="download" size="16px" />
          <span>Download {{ uiStore.mediaViewerTitle }}</span>
        </a>
      </div>

      <!-- 1. IMAGE VIEWER -->
      <div
        v-else-if="mediaType === 'image'"
        class="w-full h-full flex items-center justify-center overflow-hidden cursor-grab active:cursor-grabbing"
      >
        <img
          :src="uiStore.mediaViewerUrl"
          :alt="uiStore.mediaViewerTitle"
          @error="playbackError = true"
          :style="{
            transform: `scale(${zoomLevel}) rotate(${rotation}deg) translate(${panX}px, ${panY}px)`,
            transition: isPanning ? 'none' : 'transform 0.15s ease-out'
          }"
          class="max-w-full max-h-[82vh] object-contain rounded-lg shadow-2xl pointer-events-none"
          draggable="false"
        />
      </div>

      <!-- 2. VIDEO PLAYER WITH SEEK & HTTP RANGE -->
      <div v-else-if="mediaType === 'video'" class="w-full h-full flex items-center justify-center max-w-5xl">
        <video
          ref="videoPlayerRef"
          :src="uiStore.mediaViewerUrl"
          controls
          autoplay
          playsinline
          @error="playbackError = true"
          class="max-w-full max-h-[82vh] rounded-xl shadow-2xl bg-black"
        ></video>
      </div>

      <!-- 3. AUDIO PLAYER WITH VISUALIZER -->
      <div
        v-else-if="mediaType === 'audio'"
        class="bg-gradient-to-b from-slate-900 to-slate-950 p-8 sm:p-10 rounded-3xl border border-slate-800 shadow-2xl flex flex-col items-center space-y-6 max-w-md w-full"
      >
        <!-- Vinyl Record / Music Art -->
        <div class="w-28 h-28 rounded-full bg-gradient-to-tr from-blue-600 via-indigo-600 to-purple-600 flex items-center justify-center text-4xl shadow-xl ring-8 ring-white/5 animate-pulse">
          🎵
        </div>

        <div class="text-center space-y-1">
          <p class="font-bold text-white text-base truncate max-w-xs">{{ uiStore.mediaViewerTitle }}</p>
          <p class="text-xs text-slate-400 font-mono">Audio Playback</p>
        </div>

        <audio
          ref="audioPlayerRef"
          :src="uiStore.mediaViewerUrl"
          controls
          autoplay
          @error="playbackError = true"
          class="w-full rounded-xl"
        ></audio>
      </div>

      <!-- 4. FALLBACK / UNSUPPORTED -->
      <div v-else class="text-center text-slate-400 space-y-3">
        <div class="text-4xl">📄</div>
        <p class="text-sm font-semibold text-white">Preview not available for this format</p>
        <p class="text-xs text-slate-500">You can download the file to view it locally.</p>
        <a
          :href="uiStore.mediaViewerUrl"
          download
          class="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-xl font-medium inline-flex items-center space-x-2 shadow transition"
        >
          <FbIcon name="download" size="14px" />
          <span>Download File</span>
        </a>
      </div>
    </div>
  </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { useUiStore } from '../../stores/uiStore';

const uiStore = useUiStore();

const viewportRef = ref<HTMLElement | null>(null);
const videoPlayerRef = ref<HTMLVideoElement | null>(null);
const audioPlayerRef = ref<HTMLAudioElement | null>(null);

const zoomLevel = ref<number>(1);
const rotation = ref<number>(0);
const panX = ref<number>(0);
const panY = ref<number>(0);
const isPanning = ref<boolean>(false);
const startMouseX = ref<number>(0);
const startMouseY = ref<number>(0);

const isSpeedMenuOpen = ref<boolean>(false);
const playbackSpeed = ref<number>(1.0);
const playbackError = ref<boolean>(false);

const currentIndex = computed(() => {
  if (!uiStore.mediaViewerFile || uiStore.mediaViewerList.length === 0) return 0;
  return uiStore.mediaViewerList.findIndex((item) => item.path === uiStore.mediaViewerFile?.path);
});

const mediaType = computed(() => {
  const mime = uiStore.mediaViewerFile?.mime_type?.toLowerCase();
  if (mime) {
    if (mime.startsWith('image/')) return 'image';
    if (mime.startsWith('video/')) return 'video';
    if (mime.startsWith('audio/')) return 'audio';
  }

  const ext = uiStore.mediaViewerTitle.split('.').pop()?.toLowerCase() || '';
  if (['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp', 'bmp', 'ico', 'avif'].includes(ext)) {
    return 'image';
  }
  if (['mp4', 'webm', 'ogg', 'mov', 'mkv', 'avi'].includes(ext)) {
    return 'video';
  }
  if (['mp3', 'wav', 'flac', 'aac', 'm4a', 'opus'].includes(ext)) {
    return 'audio';
  }
  return 'other';
});

function getMediaTypeIcon(): string {
  switch (mediaType.value) {
    case 'image': return '🖼️';
    case 'video': return '🎬';
    case 'audio': return '🎵';
    default: return '📄';
  }
}

function zoomIn() {
  zoomLevel.value = Math.min(zoomLevel.value + 0.25, 4.0);
}

function zoomOut() {
  zoomLevel.value = Math.max(zoomLevel.value - 0.25, 0.25);
}

function resetZoom() {
  zoomLevel.value = 1;
  panX.value = 0;
  panY.value = 0;
}

function rotateClockwise() {
  rotation.value = (rotation.value + 90) % 360;
}

function resetImageTransform() {
  zoomLevel.value = 1;
  rotation.value = 0;
  panX.value = 0;
  panY.value = 0;
}

function startPan(e: MouseEvent) {
  if (mediaType.value !== 'image' || zoomLevel.value <= 1) return;
  isPanning.value = true;
  startMouseX.value = e.clientX - panX.value;
  startMouseY.value = e.clientY - panY.value;
}

function doPan(e: MouseEvent) {
  if (!isPanning.value) return;
  panX.value = e.clientX - startMouseX.value;
  panY.value = e.clientY - startMouseY.value;
}

function stopPan() {
  isPanning.value = false;
}

function setPlaybackSpeed(spd: number) {
  playbackSpeed.value = spd;
  if (videoPlayerRef.value) {
    videoPlayerRef.value.playbackRate = spd;
  }
  if (audioPlayerRef.value) {
    audioPlayerRef.value.playbackRate = spd;
  }
}

async function togglePiP() {
  if (!videoPlayerRef.value) return;
  if (document.pictureInPictureElement) {
    await document.exitPictureInPicture();
  } else {
    await videoPlayerRef.value.requestPictureInPicture();
  }
}

function toggleFullscreen() {
  if (!document.fullscreenElement) {
    viewportRef.value?.requestFullscreen?.();
  } else {
    document.exitFullscreen?.();
  }
}

function closeViewer() {
  uiStore.isMediaViewerOpen = false;
  resetImageTransform();
}

function handleKeyDown(e: KeyboardEvent) {
  if (!uiStore.isMediaViewerOpen) return;

  switch (e.key) {
    case 'Escape':
      closeViewer();
      break;
    case 'ArrowLeft':
      uiStore.navigateMedia('prev');
      resetImageTransform();
      break;
    case 'ArrowRight':
      uiStore.navigateMedia('next');
      resetImageTransform();
      break;
    case '+':
    case '=':
      if (mediaType.value === 'image') zoomIn();
      break;
    case '-':
    case '_':
      if (mediaType.value === 'image') zoomOut();
      break;
    case 'r':
    case 'R':
      if (mediaType.value === 'image') rotateClockwise();
      break;
    case 'f':
    case 'F':
      toggleFullscreen();
      break;
    case ' ':
      e.preventDefault();
      if (videoPlayerRef.value) {
        if (videoPlayerRef.value.paused) videoPlayerRef.value.play();
        else videoPlayerRef.value.pause();
      }
      if (audioPlayerRef.value) {
        if (audioPlayerRef.value.paused) audioPlayerRef.value.play();
        else audioPlayerRef.value.pause();
      }
      break;
  }
}

watch(
  () => uiStore.mediaViewerUrl,
  () => {
    resetImageTransform();
    playbackSpeed.value = 1.0;
    playbackError.value = false;
  }
);

onMounted(() => {
  window.addEventListener('keydown', handleKeyDown);
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown);
});
</script>

<template>
  <div
    class="w-full h-full flex items-center justify-center relative overflow-hidden bg-black select-none"
    @mousemove="handleActivity"
    @click="handleContainerClick"
  >
    <!-- Video Element -->
    <video
      ref="videoRef"
      :src="store.activeUrl"
      autoplay
      playsinline
      @loadedmetadata="handleLoadedMetadata"
      @timeupdate="handleTimeUpdate"
      @progress="handleProgress"
      @play="isPlaying = true"
      @pause="isPlaying = false"
      @ended="isPlaying = false"
      @error="store.setPlaybackError(true)"
      class="max-w-full max-h-full object-contain pointer-events-auto"
    ></video>

    <!-- Custom Floating Video Controls -->
    <VideoControls
      :visible="controlsVisible"
      :is-playing="isPlaying"
      :current-time="currentTime"
      :duration="duration"
      :buffered-time="bufferedTime"
      :volume="store.volume"
      :is-muted="store.isMuted"
      :playback-rate="playbackRate"
      :is-fullscreen="isFullscreen"
      @toggle-play="togglePlay"
      @seek="seekTo"
      @set-volume="(vol) => store.setVolume(vol)"
      @toggle-mute="store.toggleMute()"
      @set-playback-rate="setPlaybackRate"
      @toggle-pip="togglePiP"
      @toggle-fullscreen="toggleFullscreen"
      @click.stop
    />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue';
import VideoControls from './VideoControls.vue';
import { useMediaViewerStore } from '../../stores/mediaViewerStore';

const store = useMediaViewerStore();
const videoRef = ref<HTMLVideoElement | null>(null);

const isPlaying = ref(false);
const currentTime = ref(0);
const duration = ref(0);
const bufferedTime = ref(0);
const playbackRate = ref(1.0);
const isFullscreen = ref(false);

const controlsVisible = ref(true);
let idleTimer: ReturnType<typeof setTimeout> | null = null;

function handleActivity() {
  controlsVisible.value = true;
  if (idleTimer) clearTimeout(idleTimer);
  if (isPlaying.value) {
    idleTimer = setTimeout(() => {
      controlsVisible.value = false;
    }, 2500);
  }
}

function handleContainerClick(e: MouseEvent) {
  if (e.target === videoRef.value) {
    togglePlay();
  }
}

function handleLoadedMetadata() {
  if (!videoRef.value) return;
  duration.value = videoRef.value.duration;
  store.setMediaDuration(videoRef.value.duration);
  if (videoRef.value.videoWidth && videoRef.value.videoHeight) {
    store.setMediaDimensions(videoRef.value.videoWidth, videoRef.value.videoHeight);
  }
  // Synchronize stored volume to video element
  videoRef.value.volume = store.volume;
  videoRef.value.muted = store.isMuted;
}

function handleTimeUpdate() {
  if (!videoRef.value) return;
  currentTime.value = videoRef.value.currentTime;
}

function handleProgress() {
  if (!videoRef.value) return;
  if (videoRef.value.buffered.length > 0) {
    bufferedTime.value = videoRef.value.buffered.end(videoRef.value.buffered.length - 1);
  }
}

function togglePlay() {
  if (!videoRef.value) return;
  if (videoRef.value.paused) {
    void videoRef.value.play().catch(() => {});
  } else {
    videoRef.value.pause();
  }
}

function seekTo(time: number) {
  if (!videoRef.value) return;
  videoRef.value.currentTime = time;
  currentTime.value = time;
}

function setPlaybackRate(rate: number) {
  if (!videoRef.value) return;
  playbackRate.value = rate;
  videoRef.value.playbackRate = rate;
}

async function togglePiP() {
  if (!videoRef.value) return;
  try {
    if (document.pictureInPictureElement) {
      await document.exitPictureInPicture();
    } else {
      await videoRef.value.requestPictureInPicture();
    }
  } catch {}
}

function handleFullscreenChange() {
  isFullscreen.value = !!document.fullscreenElement;
}

function toggleFullscreen() {
  if (!document.fullscreenElement) {
    void videoRef.value?.requestFullscreen?.();
    isFullscreen.value = true;
  } else {
    void document.exitFullscreen?.();
    isFullscreen.value = false;
  }
}

watch(
  () => store.volume,
  (vol) => {
    if (videoRef.value) videoRef.value.volume = vol;
  }
);

watch(
  () => store.isMuted,
  (muted) => {
    if (videoRef.value) videoRef.value.muted = muted;
  }
);

watch(
  () => store.activeUrl,
  () => {
    currentTime.value = 0;
    bufferedTime.value = 0;
    isPlaying.value = false;
    playbackRate.value = 1.0;
    controlsVisible.value = true;
  }
);

onMounted(() => {
  document.addEventListener('fullscreenchange', handleFullscreenChange);
});

onUnmounted(() => {
  document.removeEventListener('fullscreenchange', handleFullscreenChange);
  if (idleTimer) clearTimeout(idleTimer);
});

defineExpose({
  togglePlay,
  toggleFullscreen,
  togglePiP,
});
</script>

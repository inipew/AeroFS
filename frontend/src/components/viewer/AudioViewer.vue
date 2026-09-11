<template>
  <div class="w-full h-full flex items-center justify-center p-4 sm:p-8 select-none">
    <!-- Mini Music Player Card -->
    <div
      class="bg-[#111317]/85 backdrop-blur-2xl border border-white/10 p-6 sm:p-8 rounded-3xl shadow-2xl flex flex-col items-center space-y-6 max-w-sm sm:max-w-md w-full relative overflow-hidden"
    >
      <!-- Background Ambient Glow -->
      <div
        class="absolute -top-24 -left-24 w-72 h-72 rounded-full blur-3xl opacity-20 pointer-events-none"
        :style="{ background: ambientGlowColor }"
      ></div>

      <!-- Hidden Audio Element -->
      <audio
        ref="audioRef"
        :src="store.activeUrl"
        autoplay
        @loadedmetadata="handleLoadedMetadata"
        @timeupdate="handleTimeUpdate"
        @play="isPlaying = true"
        @pause="isPlaying = false"
        @ended="isPlaying = false"
        @error="store.setPlaybackError(true)"
      ></audio>

      <!-- Album Artwork / Abstract Gradient Tile -->
      <div
        class="w-36 h-36 sm:w-44 sm:h-44 rounded-2xl shadow-xl flex items-center justify-center relative overflow-hidden border border-white/15 transition-transform duration-300"
        :style="artworkStyle"
        :class="[isPlaying ? 'scale-102' : 'scale-100']"
      >
        <!-- Animated Waveform Accent -->
        <div class="flex items-end justify-center space-x-1.5 h-12 w-full px-6 opacity-60">
          <div
            v-for="bar in 9"
            :key="bar"
            class="w-1.5 rounded-full bg-white transition-all duration-300"
            :style="{
              height: isPlaying ? `${15 + ((bar * 7) % 30)}px` : '6px',
              opacity: isPlaying ? 0.8 : 0.4
            }"
          ></div>
        </div>
      </div>

      <!-- Title & Format Details -->
      <div class="text-center space-y-1 w-full truncate">
        <h3 class="font-bold text-white text-base sm:text-lg truncate px-2" :title="store.activeTitle">
          {{ store.activeTitle }}
        </h3>
        <p class="text-xs text-white/50 font-mono flex items-center justify-center space-x-1.5">
          <span class="uppercase font-semibold text-blue-400">{{ extension }}</span>
          <span>·</span>
          <span>{{ formattedBytes }}</span>
        </p>
      </div>

      <!-- Scrubbing Timeline -->
      <div class="w-full space-y-1.5">
        <div
          class="relative group/timeline flex items-center h-4 cursor-pointer"
          @click="handleTimelineClick"
        >
          <!-- Background Track -->
          <div class="w-full h-1.5 group-hover/timeline:h-2 rounded-full bg-white/10 transition-all duration-150 relative overflow-hidden">
            <!-- Progress Track -->
            <div
              class="absolute inset-y-0 left-0 bg-blue-500 rounded-full"
              :style="{ width: `${progressPercent}%` }"
            ></div>
          </div>
          <!-- Scrubber Thumb -->
          <div
            class="absolute w-3.5 h-3.5 bg-white rounded-full shadow-lg -translate-x-1/2 scale-0 group-hover/timeline:scale-100 transition-transform pointer-events-none"
            :style="{ left: `${progressPercent}%` }"
          ></div>
        </div>

        <!-- Duration Numbers -->
        <div class="flex items-center justify-between text-[11px] font-mono text-white/50 px-0.5">
          <span>{{ formatTime(currentTime) }}</span>
          <span>{{ formatTime(duration) }}</span>
        </div>
      </div>

      <!-- Playback Actions: Skip -15s, Play/Pause, Skip +15s -->
      <div class="flex items-center justify-center space-x-6">
        <!-- Rewind 15s -->
        <button
          type="button"
          @click="skipTime(-15)"
          class="p-2.5 rounded-2xl hover:bg-white/10 text-white/70 hover:text-white transition cursor-pointer flex flex-col items-center"
          title="Rewind 15s"
        >
          <FbIcon name="rotate-ccw" size="18px" />
          <span class="text-[9px] font-mono mt-0.5 font-bold">15</span>
        </button>

        <!-- Main Play / Pause Button -->
        <button
          type="button"
          @click="togglePlay"
          class="w-14 h-14 rounded-2xl bg-blue-600 hover:bg-blue-500 active:scale-95 text-white flex items-center justify-center shadow-lg shadow-blue-600/30 transition-all duration-150 cursor-pointer"
          :title="isPlaying ? 'Pause (Space)' : 'Play (Space)'"
        >
          <FbIcon :name="isPlaying ? 'pause' : 'play'" size="22px" />
        </button>

        <!-- Forward 15s -->
        <button
          type="button"
          @click="skipTime(15)"
          class="p-2.5 rounded-2xl hover:bg-white/10 text-white/70 hover:text-white transition cursor-pointer flex flex-col items-center"
          title="Forward 15s"
        >
          <FbIcon name="refresh" size="18px" />
          <span class="text-[9px] font-mono mt-0.5 font-bold">15</span>
        </button>
      </div>

      <!-- Bottom Volume Control -->
      <div class="flex items-center space-x-2 pt-1">
        <button
          type="button"
          @click="store.toggleMute()"
          class="p-1.5 rounded-lg hover:bg-white/10 text-white/60 hover:text-white transition cursor-pointer"
          :title="store.isMuted || store.volume === 0 ? 'Unmute (M)' : 'Mute (M)'"
        >
          <FbIcon :name="store.isMuted || store.volume === 0 ? 'volume-x' : 'volume-2'" size="15px" />
        </button>
        <input
          type="range"
          min="0"
          max="1"
          step="0.05"
          :value="store.isMuted ? 0 : store.volume"
          @input="(e) => store.setVolume(Number((e.target as HTMLInputElement).value))"
          class="w-24 sm:w-32 h-1 accent-blue-500 bg-white/15 rounded-lg cursor-pointer transition-all opacity-60 hover:opacity-100"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { useMediaViewerStore } from '../../stores/mediaViewerStore';

const store = useMediaViewerStore();
const audioRef = ref<HTMLAudioElement | null>(null);

const isPlaying = ref(false);
const currentTime = ref(0);
const duration = ref(0);

const extension = computed(() => {
  return store.activeTitle.split('.').pop()?.toLowerCase() || 'audio';
});

const formattedBytes = computed(() => {
  const bytes = store.activeEntry?.size;
  if (!bytes) return '';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
});

const progressPercent = computed(() => {
  if (duration.value <= 0) return 0;
  return Math.min(100, Math.max(0, (currentTime.value / duration.value) * 100));
});

function hashString(str: string): number {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    hash = (hash << 5) - hash + str.charCodeAt(i);
    hash |= 0;
  }
  return Math.abs(hash);
}

const colorHue = computed(() => {
  return hashString(store.activeTitle) % 360;
});

const ambientGlowColor = computed(() => {
  return `hsl(${colorHue.value}, 75%, 50%)`;
});

const artworkStyle = computed(() => {
  const h1 = colorHue.value;
  const h2 = (h1 + 45) % 360;
  return {
    background: `linear-gradient(135deg, hsl(${h1}, 55%, 35%) 0%, hsl(${h2}, 65%, 20%) 100%)`,
  };
});

function handleLoadedMetadata() {
  if (!audioRef.value) return;
  duration.value = audioRef.value.duration;
  store.setMediaDuration(audioRef.value.duration);
  audioRef.value.volume = store.volume;
  audioRef.value.muted = store.isMuted;
}

function handleTimeUpdate() {
  if (!audioRef.value) return;
  currentTime.value = audioRef.value.currentTime;
}

function togglePlay() {
  if (!audioRef.value) return;
  if (audioRef.value.paused) {
    void audioRef.value.play().catch(() => {});
  } else {
    audioRef.value.pause();
  }
}

function skipTime(delta: number) {
  if (!audioRef.value) return;
  const next = Math.max(0, Math.min(duration.value, audioRef.value.currentTime + delta));
  audioRef.value.currentTime = next;
  currentTime.value = next;
}

function handleTimelineClick(e: MouseEvent) {
  if (!audioRef.value) return;
  const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
  const clickX = e.clientX - rect.left;
  const ratio = Math.max(0, Math.min(1, clickX / rect.width));
  const newTime = ratio * duration.value;
  audioRef.value.currentTime = newTime;
  currentTime.value = newTime;
}

watch(
  () => store.volume,
  (vol) => {
    if (audioRef.value) audioRef.value.volume = vol;
  }
);

watch(
  () => store.isMuted,
  (muted) => {
    if (audioRef.value) audioRef.value.muted = muted;
  }
);

watch(
  () => store.activeUrl,
  () => {
    currentTime.value = 0;
    isPlaying.value = false;
  }
);

function formatTime(secs: number): string {
  if (isNaN(secs) || secs < 0) return '00:00';
  const mins = Math.floor(secs / 60);
  const remainingSecs = Math.floor(secs % 60);
  return `${mins < 10 ? '0' : ''}${mins}:${remainingSecs < 10 ? '0' : ''}${remainingSecs}`;
}

defineExpose({
  togglePlay,
});
</script>

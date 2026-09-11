<template>
  <div
    class="absolute bottom-0 inset-x-0 z-30 pb-4 pt-12 px-4 sm:px-8 bg-gradient-to-t from-black/85 via-black/40 to-transparent flex flex-col space-y-2 select-none pointer-events-auto transition-opacity duration-300"
    :class="[visible ? 'opacity-100' : 'opacity-0 pointer-events-none']"
  >
    <!-- Scrubbing Timeline Slider -->
    <div class="relative group/timeline flex items-center h-4 cursor-pointer" @click="handleTimelineClick">
      <!-- Background Track -->
      <div class="w-full h-1 group-hover/timeline:h-2 rounded-full bg-white/20 transition-all duration-150 relative overflow-hidden">
        <!-- Buffered Track -->
        <div
          class="absolute inset-y-0 left-0 bg-white/30 rounded-full"
          :style="{ width: `${bufferedPercent}%` }"
        ></div>
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

    <!-- Controls Row -->
    <div class="flex items-center justify-between text-xs text-white/90">
      <!-- Left: Play/Pause & Time -->
      <div class="flex items-center space-x-3">
        <!-- Play / Pause -->
        <button
          type="button"
          @click="$emit('togglePlay')"
          class="p-2 rounded-xl bg-white/10 hover:bg-white/20 text-white transition cursor-pointer"
          :title="isPlaying ? 'Pause (Space)' : 'Play (Space)'"
        >
          <FbIcon :name="isPlaying ? 'pause' : 'play'" size="16px" />
        </button>

        <!-- Time: Elapsed / Total -->
        <div class="font-mono text-[11px] text-white/70 space-x-1">
          <span class="text-white font-semibold">{{ formatTime(currentTime) }}</span>
          <span>/</span>
          <span>{{ formatTime(duration) }}</span>
        </div>
      </div>

      <!-- Right: Volume, Speed, PiP, Fullscreen -->
      <div class="flex items-center space-x-1 sm:space-x-2">
        <!-- Volume Slider & Mute Toggle -->
        <div class="flex items-center space-x-1.5 group/vol">
          <button
            type="button"
            @click="$emit('toggleMute')"
            class="p-1.5 rounded-lg hover:bg-white/10 text-white/80 hover:text-white transition cursor-pointer"
            :title="isMuted || volume === 0 ? 'Unmute (M)' : 'Mute (M)'"
          >
            <FbIcon :name="isMuted || volume === 0 ? 'volume-x' : 'volume-2'" size="16px" />
          </button>
          <input
            type="range"
            min="0"
            max="1"
            step="0.05"
            :value="isMuted ? 0 : volume"
            @input="(e) => $emit('setVolume', Number((e.target as HTMLInputElement).value))"
            class="w-16 sm:w-20 h-1 accent-blue-500 bg-white/20 rounded-lg cursor-pointer transition-all opacity-70 hover:opacity-100"
          />
        </div>

        <!-- Playback Speed Dropdown -->
        <div class="relative">
          <button
            type="button"
            @click="isSpeedOpen = !isSpeedOpen"
            class="px-2 py-1 rounded-lg hover:bg-white/10 font-mono text-[11px] font-semibold text-white/80 hover:text-white transition cursor-pointer"
            title="Playback Speed"
          >
            {{ playbackRate }}×
          </button>

          <div
            v-if="isSpeedOpen"
            class="absolute bottom-full right-0 mb-2 py-1 bg-black/90 border border-white/15 rounded-xl shadow-2xl backdrop-blur-xl z-50 text-[11px] font-mono space-y-0.5 min-w-[72px]"
          >
            <button
              v-for="spd in [0.5, 0.75, 1.0, 1.25, 1.5, 2.0]"
              :key="spd"
              type="button"
              @click="$emit('setPlaybackRate', spd); isSpeedOpen = false;"
              class="w-full text-left px-2.5 py-1 hover:bg-white/15 transition cursor-pointer"
              :class="playbackRate === spd ? 'text-blue-400 font-bold bg-white/10' : 'text-white/70'"
            >
              {{ spd }}×
            </button>
          </div>
        </div>

        <!-- PiP -->
        <button
          type="button"
          @click="$emit('togglePiP')"
          class="p-1.5 rounded-lg hover:bg-white/10 text-white/80 hover:text-white transition cursor-pointer hidden sm:flex"
          title="Picture-in-Picture"
        >
          <FbIcon name="panel-right" size="15px" />
        </button>

        <!-- Fullscreen -->
        <button
          type="button"
          @click="$emit('toggleFullscreen')"
          class="p-1.5 rounded-lg hover:bg-white/10 text-white/80 hover:text-white transition cursor-pointer"
          title="Toggle Fullscreen (F)"
        >
          <FbIcon :name="isFullscreen ? 'minimize' : 'maximize'" size="15px" />
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import FbIcon from '../common/FbIcon.vue';

const props = defineProps<{
  visible: boolean;
  isPlaying: boolean;
  currentTime: number;
  duration: number;
  bufferedTime: number;
  volume: number;
  isMuted: boolean;
  playbackRate: number;
  isFullscreen: boolean;
}>();

const emit = defineEmits<{
  (e: 'togglePlay'): void;
  (e: 'seek', time: number): void;
  (e: 'setVolume', vol: number): void;
  (e: 'toggleMute'): void;
  (e: 'setPlaybackRate', rate: number): void;
  (e: 'togglePiP'): void;
  (e: 'toggleFullscreen'): void;
}>();

const isSpeedOpen = ref(false);

const progressPercent = computed(() => {
  if (props.duration <= 0) return 0;
  return Math.min(100, Math.max(0, (props.currentTime / props.duration) * 100));
});

const bufferedPercent = computed(() => {
  if (props.duration <= 0) return 0;
  return Math.min(100, Math.max(0, (props.bufferedTime / props.duration) * 100));
});

function handleTimelineClick(e: MouseEvent) {
  const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
  const clickX = e.clientX - rect.left;
  const ratio = Math.max(0, Math.min(1, clickX / rect.width));
  emit('seek', ratio * props.duration);
}

function formatTime(secs: number): string {
  if (isNaN(secs) || secs < 0) return '00:00';
  const mins = Math.floor(secs / 60);
  const remainingSecs = Math.floor(secs % 60);
  return `${mins < 10 ? '0' : ''}${mins}:${remainingSecs < 10 ? '0' : ''}${remainingSecs}`;
}
</script>

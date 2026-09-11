<template>
  <header
    class="h-14 px-3.5 sm:px-6 flex items-center justify-between gap-3 text-xs shrink-0 select-none z-30 transition-opacity duration-200"
    :class="[
      isFloating ? 'absolute top-0 inset-x-0 bg-gradient-to-b from-black/80 via-black/40 to-transparent pointer-events-auto' : 'bg-[#08090b]/90 border-b border-white/[0.08]'
    ]"
  >
    <!-- Left: File Icon, Title & Dynamic Metadata Subtitle -->
    <div class="flex items-center space-x-3 min-w-0">
      <!-- Media Type Icon Pill -->
      <div
        class="w-9 h-9 rounded-xl flex items-center justify-center shrink-0 border"
        :class="iconContainerClass"
      >
        <FbIcon :name="typeIcon" size="16px" />
      </div>

      <div class="min-w-0">
        <div class="flex items-center space-x-2 truncate">
          <h2
            class="text-xs sm:text-sm font-bold text-white truncate tracking-tight"
            :title="store.activeTitle"
          >
            {{ store.activeTitle }}
          </h2>

          <!-- Playlist Position Indicator -->
          <span
            v-if="store.hasMultiple"
            class="px-2 py-0.5 rounded-full text-[10px] font-mono font-semibold bg-white/10 text-white/70 border border-white/10 shrink-0 hidden sm:inline-block"
          >
            {{ store.currentIndex + 1 }} / {{ store.playlist.length }}
          </span>
        </div>

        <!-- Subtitle: Dimensions / Duration / Format / File Size -->
        <p class="text-[11px] text-white/50 font-mono truncate mt-0.5 flex items-center space-x-1.5">
          <span v-if="metadataSummary">{{ metadataSummary }} · </span>
          <span class="uppercase font-semibold text-white/65">{{ extension }}</span>
          <span v-if="formattedSize"> · {{ formattedSize }}</span>
        </p>
      </div>
    </div>

    <!-- Right Controls: Desktop Toolbar & Mobile 'More' Menu -->
    <div class="flex items-center space-x-1 sm:space-x-1.5 shrink-0">
      <!-- Desktop: Info Drawer Toggle -->
      <button
        type="button"
        @click="store.toggleInfo()"
        :class="[
          'p-2 rounded-xl transition cursor-pointer hidden sm:flex items-center justify-center',
          store.isInfoOpen
            ? 'bg-blue-600/30 text-blue-400 border border-blue-500/40'
            : 'text-white/70 hover:text-white hover:bg-white/10'
        ]"
        title="File Information (I)"
      >
        <FbIcon name="info" size="15px" />
      </button>

      <!-- Desktop: Filmstrip / Gallery Toggle -->
      <button
        v-if="store.hasMultiple"
        type="button"
        @click="store.toggleFilmstrip()"
        :class="[
          'p-2 rounded-xl transition cursor-pointer hidden sm:flex items-center justify-center',
          store.isFilmstripOpen
            ? 'bg-blue-600/30 text-blue-400 border border-blue-500/40'
            : 'text-white/70 hover:text-white hover:bg-white/10'
        ]"
        title="Toggle Filmstrip (G)"
      >
        <FbIcon name="layers" size="15px" />
      </button>

      <!-- Desktop: Download Secondary Button -->
      <a
        :href="store.downloadUrl"
        :download="store.activeTitle"
        class="p-2 text-white/70 hover:text-white hover:bg-white/10 rounded-xl transition cursor-pointer hidden sm:flex items-center justify-center"
        title="Download File"
      >
        <FbIcon name="download" size="15px" />
      </a>

      <!-- Desktop: Fullscreen Toggle -->
      <button
        type="button"
        @click="$emit('toggleFullscreen')"
        class="p-2 text-white/70 hover:text-white hover:bg-white/10 rounded-xl transition cursor-pointer hidden sm:flex items-center justify-center"
        title="Toggle Fullscreen (F)"
      >
        <FbIcon :name="isFullscreen ? 'minimize' : 'maximize'" size="15px" />
      </button>

      <!-- Mobile: More Actions Sheet Button (⋯) -->
      <button
        type="button"
        @click="store.toggleMobileMenu()"
        class="p-2 text-white/70 hover:text-white hover:bg-white/10 rounded-xl transition cursor-pointer sm:hidden flex items-center justify-center"
        title="More Actions"
      >
        <FbIcon name="more-horizontal" size="16px" />
      </button>

      <!-- Close Button (Always Visible) -->
      <button
        type="button"
        @click="store.close()"
        class="p-2 text-white/70 hover:text-white hover:bg-red-500/20 hover:text-red-400 rounded-xl transition cursor-pointer ml-1 flex items-center justify-center"
        title="Close (Esc)"
      >
        <FbIcon name="x" size="16px" />
      </button>
    </div>
  </header>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { useMediaViewerStore } from '../../stores/mediaViewerStore';

const props = withDefaults(
  defineProps<{
    isFloating?: boolean;
    isFullscreen?: boolean;
  }>(),
  {
    isFloating: true,
    isFullscreen: false,
  }
);

defineEmits<{
  (e: 'toggleFullscreen'): void;
}>();

const store = useMediaViewerStore();

const extension = computed(() => {
  return store.activeTitle.split('.').pop()?.toLowerCase() || '';
});

const formattedSize = computed(() => {
  const bytes = store.activeEntry?.size;
  if (typeof bytes !== 'number' || bytes <= 0) return '';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
});

const metadataSummary = computed(() => {
  if (store.mediaDimensions) {
    return `${store.mediaDimensions.width} × ${store.mediaDimensions.height}`;
  }
  if (store.mediaDuration) {
    const mins = Math.floor(store.mediaDuration / 60);
    const secs = Math.floor(store.mediaDuration % 60);
    return `${mins}:${secs < 10 ? '0' : ''}${secs}`;
  }
  return '';
});

const typeIcon = computed(() => {
  switch (store.activeMediaKind) {
    case 'image': return 'image';
    case 'video': return 'video';
    case 'audio': return 'music';
    default: return 'file';
  }
});

const iconContainerClass = computed(() => {
  switch (store.activeMediaKind) {
    case 'image':
      return 'bg-amber-500/10 text-amber-400 border-amber-500/20';
    case 'video':
      return 'bg-purple-500/10 text-purple-400 border-purple-500/20';
    case 'audio':
      return 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20';
    default:
      return 'bg-blue-500/10 text-blue-400 border-blue-500/20';
  }
});
</script>

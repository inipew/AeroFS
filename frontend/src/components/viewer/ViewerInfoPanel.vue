<template>
  <div v-if="store.isInfoOpen" class="contents select-none">
    <!-- Mobile Dismissal Backdrop Overlay -->
    <div
      class="fixed inset-0 z-40 bg-black/60 backdrop-blur-xs sm:hidden pointer-events-auto"
      @click="store.toggleInfo(false)"
    ></div>

    <Transition name="slide-panel">
      <aside
        class="fixed inset-y-0 right-0 z-50 w-80 sm:w-88 bg-[#0c0e12]/95 backdrop-blur-xl border-l border-white/10 shadow-2xl flex flex-col text-xs text-white/80 select-none pointer-events-auto"
      >
        <!-- Panel Header -->
        <div class="h-14 px-5 border-b border-white/[0.08] flex items-center justify-between shrink-0">
          <div class="flex items-center space-x-2">
            <FbIcon name="info" size="16px" class="text-blue-400" />
            <h3 class="text-sm font-bold text-white tracking-tight">File Information</h3>
          </div>
          <button
            type="button"
            @click="store.toggleInfo(false)"
            class="p-1.5 rounded-lg text-white/60 hover:text-white hover:bg-white/10 transition cursor-pointer"
            title="Close (Esc)"
          >
            <FbIcon name="x" size="16px" />
          </button>
        </div>

        <!-- Panel Body: Metadata Fields -->
        <div class="flex-1 overflow-y-auto p-5 space-y-4">
          <!-- File Name & Kind -->
          <div>
            <span class="text-[10px] font-semibold uppercase tracking-wider text-white/40 block mb-1">Name</span>
            <p class="text-sm font-bold text-white break-words">{{ store.activeTitle }}</p>
          </div>

          <div class="h-[1px] bg-white/[0.06]"></div>

          <!-- Format & MIME -->
          <div class="grid grid-cols-2 gap-3">
            <div>
              <span class="text-[10px] font-semibold uppercase tracking-wider text-white/40 block mb-1">Kind</span>
              <p class="font-medium text-white/90 capitalize">{{ store.activeMediaKind }}</p>
            </div>
            <div>
              <span class="text-[10px] font-semibold uppercase tracking-wider text-white/40 block mb-1">Format</span>
              <p class="font-mono font-semibold uppercase text-blue-400">{{ extension }}</p>
            </div>
          </div>

          <!-- Dimensions / Duration (If Available) -->
          <div v-if="dimensionsString || durationString" class="grid grid-cols-2 gap-3">
            <div v-if="dimensionsString">
              <span class="text-[10px] font-semibold uppercase tracking-wider text-white/40 block mb-1">Dimensions</span>
              <p class="font-mono text-white/90">{{ dimensionsString }}</p>
            </div>
            <div v-if="durationString">
              <span class="text-[10px] font-semibold uppercase tracking-wider text-white/40 block mb-1">Duration</span>
              <p class="font-mono text-white/90">{{ durationString }}</p>
            </div>
          </div>

          <!-- File Size -->
          <div>
            <span class="text-[10px] font-semibold uppercase tracking-wider text-white/40 block mb-1">Size</span>
            <p class="font-mono text-white/90">
              {{ formattedBytes }}
              <span v-if="rawBytes" class="text-white/40 text-[11px]">({{ rawBytes.toLocaleString() }} bytes)</span>
            </p>
          </div>

          <!-- Modified Date -->
          <div v-if="store.activeEntry?.modified_at">
            <span class="text-[10px] font-semibold uppercase tracking-wider text-white/40 block mb-1">Modified</span>
            <p class="font-mono text-white/80 text-[11px]">{{ formattedDate }}</p>
          </div>

          <!-- Location / Path -->
          <div>
            <span class="text-[10px] font-semibold uppercase tracking-wider text-white/40 block mb-1">Storage Path</span>
            <div class="p-2 rounded-xl bg-white/[0.04] border border-white/[0.06] font-mono text-[11px] text-white/70 break-all select-text">
              <span class="text-blue-400 font-semibold">{{ store.connectionId }}:</span>{{ store.activeEntry?.path }}
            </div>
          </div>
        </div>

        <!-- Panel Footer: Quick Actions -->
        <div class="p-4 border-t border-white/[0.08] space-y-2 bg-[#090a0c] shrink-0">
          <button
            type="button"
            @click="copyPath"
            class="w-full py-2 px-3 rounded-xl bg-white/10 hover:bg-white/15 text-white font-medium flex items-center justify-center space-x-2 transition cursor-pointer"
          >
            <FbIcon :name="copiedPath ? 'check' : 'clipboard'" size="14px" :class="copiedPath ? 'text-emerald-400' : ''" />
            <span>{{ copiedPath ? 'Path Copied!' : 'Copy Path' }}</span>
          </button>

          <a
            :href="store.downloadUrl"
            :download="store.activeTitle"
            class="w-full py-2 px-3 rounded-xl bg-blue-600 hover:bg-blue-700 text-white font-semibold flex items-center justify-center space-x-2 transition cursor-pointer shadow-md"
          >
            <FbIcon name="download" size="14px" />
            <span>Download File</span>
          </a>
        </div>
      </aside>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { useMediaViewerStore } from '../../stores/mediaViewerStore';

const store = useMediaViewerStore();
const copiedPath = ref(false);

const extension = computed(() => {
  return store.activeTitle.split('.').pop()?.toLowerCase() || 'unknown';
});

const rawBytes = computed(() => {
  return store.activeEntry?.size || 0;
});

const formattedBytes = computed(() => {
  const bytes = rawBytes.value;
  if (!bytes) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
});

const dimensionsString = computed(() => {
  if (!store.mediaDimensions) return '';
  return `${store.mediaDimensions.width} × ${store.mediaDimensions.height}`;
});

const durationString = computed(() => {
  if (!store.mediaDuration) return '';
  const mins = Math.floor(store.mediaDuration / 60);
  const secs = Math.floor(store.mediaDuration % 60);
  return `${mins}:${secs < 10 ? '0' : ''}${secs}`;
});

const formattedDate = computed(() => {
  const d = store.activeEntry?.modified_at;
  if (!d) return '-';
  try {
    return new Date(d).toLocaleString(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    });
  } catch {
    return d;
  }
});

async function copyPath() {
  if (!store.activeEntry?.path) return;
  try {
    await navigator.clipboard.writeText(store.activeEntry.path);
    copiedPath.value = true;
    setTimeout(() => {
      copiedPath.value = false;
    }, 2000);
  } catch {}
}
</script>

<style scoped>
.slide-panel-enter-active,
.slide-panel-leave-active {
  transition: transform 0.25s cubic-bezier(0.16, 1, 0.3, 1);
}

.slide-panel-enter-from,
.slide-panel-leave-to {
  transform: translateX(100%);
}
</style>

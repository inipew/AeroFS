<template>
  <nav
    class="breadcrumb-capsule flex items-center space-x-1 text-xs select-none overflow-x-auto no-scrollbar shadow-2xs flex-1 min-w-0"
    @dblclick="$emit('editPath')"
  >
    <!-- Root Item -->
    <button
      type="button"
      @click="$emit('navigate', '/')"
      :class="[
        'px-2 py-0.5 rounded-lg transition-[background-color,color,transform] duration-fast ease-spring flex items-center space-x-1.5 shrink-0 active:scale-95 cursor-pointer font-medium',
        path === '/'
          ? 'text-blue-600 dark:text-blue-400 font-semibold bg-blue-50/80 dark:bg-blue-950/40'
          : 'text-gray-600 dark:text-slate-400 hover:text-gray-900 dark:hover:text-white hover:bg-gray-200/60 dark:hover:bg-slate-700/60'
      ]"
      title="Root (/)"
    >
      <FbIcon :name="connectionProvider === 'local' ? 'folder' : 'share'" size="13px" class="text-blue-500 shrink-0" />
      <span class="truncate max-w-[80px] sm:max-w-[120px]">{{ connectionName }}</span>
      <span
        v-if="connectionStatus"
        :class="[
          'w-2 h-2 rounded-full shrink-0 ml-1',
          connectionStatus === 'connected'
            ? 'bg-emerald-500'
            : connectionStatus === 'orphaned'
            ? 'bg-red-500 ring-2 ring-red-200'
            : connectionStatus === 'offline'
            ? 'bg-gray-400'
            : 'bg-amber-500'
        ]"
        :title="`Status: ${connectionStatus}`"
      ></span>
    </button>

    <!-- Intermediate Truncated Popover (if depth > maxVisible) -->
    <div v-if="hasTruncatedSegments" ref="truncatedMenuRef" class="relative shrink-0 flex items-center">
      <span class="text-gray-400 dark:text-slate-600 font-bold text-xs shrink-0 select-none">›</span>
      <button
        type="button"
        @click="isTruncatedOpen = !isTruncatedOpen"
        class="px-1.5 py-0.5 rounded-md text-gray-500 hover:text-gray-900 dark:text-slate-400 dark:hover:text-white hover:bg-gray-200/60 dark:hover:bg-slate-700/60 transition cursor-pointer text-xs font-bold"
        title="Show intermediate folders"
      >
        ...
      </button>

      <!-- Truncated Paths Popover Menu -->
      <Transition name="ios-popover">
        <div
          v-if="isTruncatedOpen"
          @click="isTruncatedOpen = false"
          class="absolute left-0 top-full mt-1.5 w-52 bg-white/95 dark:bg-[#0f172a]/95 backdrop-blur-2xl border border-gray-200/90 dark:border-slate-700/80 rounded-2xl shadow-2xl ring-1 ring-black/5 dark:ring-white/10 p-1.5 z-50 text-xs space-y-0.5"
        >
          <button
            v-for="seg in truncatedSegments"
            :key="seg.path"
            type="button"
            @click="$emit('navigate', seg.path)"
            class="w-full flex items-center space-x-2 px-2.5 py-1.5 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800 transition text-left text-gray-700 dark:text-slate-300 cursor-pointer"
          >
            <FbIcon name="folder" size="13px" class="text-amber-500 shrink-0" />
            <span class="truncate">{{ seg.name }}</span>
          </button>
        </div>
      </Transition>
    </div>

    <!-- Visible Segments -->
    <TransitionGroup name="crumb-item" tag="div" class="flex items-center space-x-1 shrink-0">
      <div v-for="(seg, idx) in visibleSegments" :key="seg.path" class="flex items-center space-x-1">
        <span class="text-gray-400 dark:text-slate-600 font-bold text-xs shrink-0 select-none">›</span>
        <button
          type="button"
          @click="$emit('navigate', seg.path)"
          :class="[
            'px-2 py-0.5 rounded-lg transition-[background-color,color,transform] duration-fast ease-spring max-w-[110px] sm:max-w-[160px] truncate active:scale-95 cursor-pointer',
            idx === visibleSegments.length - 1 && !hasTruncatedSegmentsAfter
              ? 'text-gray-900 dark:text-white font-bold bg-gray-200/70 dark:bg-slate-700/60'
              : 'text-gray-600 dark:text-slate-400 hover:text-gray-900 dark:hover:text-white hover:bg-gray-200/60 dark:hover:bg-slate-700/60 font-medium'
          ]"
          :title="seg.path"
        >
          {{ seg.name }}
        </button>
      </div>
    </TransitionGroup>
  </nav>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import FbIcon from '../../../components/common/FbIcon.vue';
import { getPathSegments } from '../../../utils/path';

const props = withDefaults(
  defineProps<{
    path: string;
    connectionName?: string;
    connectionProvider?: string;
    connectionStatus?: string;
    maxVisibleSegments?: number;
  }>(),
  {
    connectionName: 'Local Storage',
    connectionProvider: 'local',
    connectionStatus: 'connected',
    maxVisibleSegments: 3,
  }
);

defineEmits<{
  (e: 'navigate', path: string): void;
  (e: 'editPath'): void;
}>();

const isTruncatedOpen = ref(false);
const truncatedMenuRef = ref<HTMLElement | null>(null);

function handlePointerDown(e: PointerEvent) {
  if (isTruncatedOpen.value && truncatedMenuRef.value && !truncatedMenuRef.value.contains(e.target as Node)) {
    isTruncatedOpen.value = false;
  }
}

function handleKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape' && isTruncatedOpen.value) {
    isTruncatedOpen.value = false;
  }
}

onMounted(() => {
  window.addEventListener('pointerdown', handlePointerDown);
  window.addEventListener('keydown', handleKeyDown);
});

onUnmounted(() => {
  window.removeEventListener('pointerdown', handlePointerDown);
  window.removeEventListener('keydown', handleKeyDown);
});

interface CrumbSegment {
  name: string;
  path: string;
}

const allSegments = computed<CrumbSegment[]>(() => {
  const segs = getPathSegments(props.path);
  // Omit root '/' since it's dedicated button
  return segs.filter((s) => s.path !== '/');
});

const hasTruncatedSegments = computed(() => {
  return allSegments.value.length > props.maxVisibleSegments;
});

const hasTruncatedSegmentsAfter = computed(() => false);

const truncatedSegments = computed(() => {
  if (!hasTruncatedSegments.value) return [];
  // intermediate segments: from 0 to length - maxVisible
  return allSegments.value.slice(0, allSegments.value.length - (props.maxVisibleSegments - 1));
});

const visibleSegments = computed(() => {
  if (!hasTruncatedSegments.value) return allSegments.value;
  return allSegments.value.slice(allSegments.value.length - (props.maxVisibleSegments - 1));
});
</script>

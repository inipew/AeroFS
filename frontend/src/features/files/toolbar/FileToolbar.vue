<template>
  <div class="flex items-center space-x-1.5 sm:space-x-2 shrink-0 relative">
    <!-- View Mode Switcher (List vs Grid) -->
    <div class="ios-segmented-group shrink-0">
      <button
        type="button"
        @click="$emit('update:viewMode', 'list')"
        :class="['ios-segmented-item p-1.5', viewMode === 'list' ? 'active' : '']"
        title="List View"
      >
        <FbIcon name="list" size="13px" />
      </button>
      <button
        type="button"
        @click="$emit('update:viewMode', 'grid')"
        :class="['ios-segmented-item p-1.5', viewMode === 'grid' ? 'active' : '']"
        title="Grid View"
      >
        <FbIcon name="grid" size="13px" />
      </button>
    </div>

    <!-- Refresh Button -->
    <button
      type="button"
      @click="$emit('refresh')"
      class="p-1.5 rounded-xl border border-gray-200/90 dark:border-slate-800/90 bg-white dark:bg-slate-900 text-gray-600 dark:text-slate-400 hover:bg-gray-50 dark:hover:bg-slate-800 hover:text-gray-900 dark:hover:text-white transition cursor-pointer active:scale-95 duration-fast ease-spring shadow-2xs"
      title="Refresh Directory (F5)"
    >
      <FbIcon name="refresh" size="13px" :class="{ 'animate-spin': isRefreshing }" />
    </button>

    <!-- Sort & View Options Popover Button -->
    <div ref="sortMenuRef" class="relative">
      <button
        type="button"
        @click="toggleSort"
        :class="[
          'p-1.5 rounded-xl border transition cursor-pointer active:scale-95 duration-fast ease-spring flex items-center space-x-1 shadow-2xs',
          isSortOpen || showHidden || sortField !== 'name'
            ? 'text-blue-600 dark:text-blue-400 border-blue-500/50 bg-blue-50/80 dark:bg-blue-950/60 ring-2 ring-blue-500/20 shadow-xs'
            : 'border-gray-200/90 dark:border-slate-800/90 bg-white dark:bg-slate-900 text-gray-600 dark:text-slate-400 hover:bg-gray-50 dark:hover:bg-slate-800 hover:text-gray-900 dark:hover:text-white'
        ]"
        title="View & Sort Options"
      >
        <FbIcon name="sort" size="13px" />
      </button>

      <!-- Unified View Popover Menu -->
      <Transition name="ios-popover">
        <div
          v-if="isSortOpen"
          @click.stop
          class="absolute right-0 mt-2.5 w-60 sm:w-64 max-w-[calc(100vw-2rem)] bg-white/95 dark:bg-[#0f172a]/95 backdrop-blur-2xl border border-gray-200/90 dark:border-slate-700/80 rounded-2xl shadow-2xl ring-1 ring-black/5 dark:ring-white/10 p-2.5 z-50 text-xs text-gray-700 dark:text-slate-200 space-y-2.5"
        >
          <!-- SECTION 1: VIEW & VISIBILITY -->
          <div>
            <div class="px-2 py-1 text-[10px] font-bold text-gray-400 dark:text-slate-400 uppercase tracking-widest flex items-center justify-between">
              <span>View & Visibility</span>
            </div>
            <div class="space-y-0.5 mt-1">
              <button
                type="button"
                @click="$emit('update:showHidden', !showHidden)"
                class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-xl hover:bg-gray-100/90 dark:hover:bg-slate-800/80 transition text-left cursor-pointer group"
              >
                <div class="flex items-center space-x-2.5">
                  <FbIcon name="eye" size="14px" class="text-gray-400 group-hover:text-blue-500 transition-colors" />
                  <span class="font-medium text-gray-700 dark:text-slate-200">Show Hidden Dotfiles</span>
                </div>
                <span
                  v-if="showHidden"
                  class="w-4 h-4 rounded-full bg-blue-500/15 dark:bg-blue-400/20 text-blue-600 dark:text-blue-400 flex items-center justify-center text-[10px] font-bold"
                >
                  ✓
                </span>
              </button>

              <button
                type="button"
                @click="$emit('selectAll'); closeMenu()"
                class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-xl hover:bg-gray-100/90 dark:hover:bg-slate-800/80 transition text-left cursor-pointer group"
              >
                <div class="flex items-center space-x-2.5">
                  <FbIcon name="select-all" size="14px" class="text-gray-400 group-hover:text-blue-500 transition-colors" />
                  <span class="font-medium text-gray-700 dark:text-slate-200">Select All Items</span>
                </div>
                <kbd class="px-1.5 py-0.5 rounded-md bg-gray-100 dark:bg-slate-800 text-[10px] font-mono text-gray-400 dark:text-slate-400 border border-gray-200/60 dark:border-slate-700/60 shadow-2xs">⌘A</kbd>
              </button>
            </div>
          </div>

          <div class="border-t border-gray-100 dark:border-slate-800/80"></div>

          <!-- SECTION 2: SORT BY -->
          <div>
            <div class="px-2 py-1 text-[10px] font-bold text-gray-400 dark:text-slate-400 uppercase tracking-widest">
              Sort By
            </div>
            <div class="space-y-0.5 mt-1">
              <button
                v-for="field in sortFields"
                :key="field.id"
                type="button"
                @click="$emit('update:sortField', field.id)"
                :class="[
                  'w-full flex items-center justify-between px-2.5 py-1.5 rounded-xl transition text-left cursor-pointer',
                  sortField === field.id
                    ? 'bg-blue-50 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400 font-semibold'
                    : 'hover:bg-gray-100/90 dark:hover:bg-slate-800/80 text-gray-700 dark:text-slate-300'
                ]"
              >
                <span>{{ field.label }}</span>
                <span
                  v-if="sortField === field.id"
                  class="w-4 h-4 rounded-full bg-blue-500/15 dark:bg-blue-400/20 text-blue-600 dark:text-blue-400 flex items-center justify-center text-[10px] font-bold"
                >
                  ✓
                </span>
              </button>
            </div>
          </div>

          <!-- SECTION 3: ORDER (ASC / DESC) -->
          <div>
            <div class="grid grid-cols-2 gap-1 p-0.5 bg-gray-100 dark:bg-slate-800/90 rounded-xl border border-gray-200/50 dark:border-slate-700/50">
              <button
                type="button"
                @click="$emit('update:sortOrder', 'asc')"
                :class="[
                  'py-1 rounded-lg transition text-center font-medium cursor-pointer text-xs',
                  sortOrder === 'asc'
                    ? 'bg-white dark:bg-slate-900 text-blue-600 dark:text-blue-400 shadow-2xs font-semibold'
                    : 'text-gray-500 dark:text-slate-400 hover:text-gray-900 dark:hover:text-white'
                ]"
              >
                Ascending ↑
              </button>
              <button
                type="button"
                @click="$emit('update:sortOrder', 'desc')"
                :class="[
                  'py-1 rounded-lg transition text-center font-medium cursor-pointer text-xs',
                  sortOrder === 'desc'
                    ? 'bg-white dark:bg-slate-900 text-blue-600 dark:text-blue-400 shadow-2xs font-semibold'
                    : 'text-gray-500 dark:text-slate-400 hover:text-gray-900 dark:hover:text-white'
                ]"
              >
                Descending ↓
              </button>
            </div>
          </div>

          <div class="border-t border-gray-100 dark:border-slate-800/80"></div>

          <!-- SECTION 4: PATH & SHORTCUTS -->
          <div class="space-y-0.5">
            <button
              type="button"
              @click="$emit('editPath'); closeMenu()"
              class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-xl hover:bg-gray-100/90 dark:hover:bg-slate-800/80 transition text-left cursor-pointer group"
            >
              <div class="flex items-center space-x-2.5">
                <FbIcon name="rename" size="14px" class="text-gray-400 group-hover:text-blue-500 transition-colors" />
                <span class="font-medium text-gray-700 dark:text-slate-200">Edit Path Directly</span>
              </div>
              <kbd class="px-1.5 py-0.5 rounded-md bg-gray-100 dark:bg-slate-800 text-[10px] font-mono text-gray-400 dark:text-slate-400 border border-gray-200/60 dark:border-slate-700/60 shadow-2xs">⌘L</kbd>
            </button>
            <button
              type="button"
              @click="$emit('copyPath'); closeMenu()"
              class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-xl hover:bg-gray-100/90 dark:hover:bg-slate-800/80 transition text-left cursor-pointer group"
            >
              <div class="flex items-center space-x-2.5">
                <FbIcon name="copy" size="14px" class="text-gray-400 group-hover:text-blue-500 transition-colors" />
                <span class="font-medium text-gray-700 dark:text-slate-200">Copy Current Path</span>
              </div>
            </button>
            <button
              v-if="isDualPane"
              type="button"
              @click="$emit('swapPanels'); closeMenu()"
              class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-xl hover:bg-gray-100/90 dark:hover:bg-slate-800/80 transition text-left cursor-pointer group"
            >
              <div class="flex items-center space-x-2.5">
                <span class="text-xs text-gray-400 group-hover:text-blue-500 transition-colors">⇄</span>
                <span class="font-medium text-gray-700 dark:text-slate-200">Swap Panels</span>
              </div>
              <kbd class="px-1.5 py-0.5 rounded-md bg-gray-100 dark:bg-slate-800 text-[10px] font-mono text-gray-400 dark:text-slate-400 border border-gray-200/60 dark:border-slate-700/60 shadow-2xs">Alt+S</kbd>
            </button>
          </div>
        </div>
      </Transition>
    </div>

    <!-- PRIMARY ACTION: + New Button (Filled Blue Pill) -->
    <div ref="newMenuRef" class="relative">
      <button
        type="button"
        @click="toggleNew"
        :class="[
          'text-white font-semibold px-2.5 sm:px-3 py-1.5 rounded-xl flex items-center space-x-1.5 text-xs shadow-xs transition cursor-pointer active:scale-95 duration-fast ease-spring',
          isNewOpen
            ? 'bg-blue-700 ring-2 ring-blue-500/40 shadow-sm'
            : 'bg-blue-600 hover:bg-blue-500 active:bg-blue-700'
        ]"
      >
        <FbIcon name="plus" size="13px" />
        <span class="hidden sm:inline">New</span>
      </button>

      <!-- New Item Dropdown Menu -->
      <Transition name="ios-popover">
        <div
          v-if="isNewOpen"
          @click.stop
          class="absolute right-0 mt-2.5 w-48 sm:w-52 bg-white/95 dark:bg-[#0f172a]/95 backdrop-blur-2xl border border-gray-200/90 dark:border-slate-700/80 rounded-2xl shadow-2xl ring-1 ring-black/5 dark:ring-white/10 p-1.5 z-50 text-xs text-gray-700 dark:text-slate-200 space-y-0.5"
        >
          <button
            type="button"
            @click="$emit('newFile'); closeMenu()"
            class="w-full flex items-center space-x-2.5 px-2.5 py-2 rounded-xl hover:bg-gray-100/90 dark:hover:bg-slate-800/80 transition text-left cursor-pointer group"
          >
            <div class="w-7 h-7 rounded-xl bg-blue-500/10 text-blue-600 dark:text-blue-400 flex items-center justify-center shrink-0 border border-blue-500/20 group-hover:scale-105 transition-transform">
              <FbIcon name="new-file" size="14px" />
            </div>
            <div class="flex flex-col min-w-0">
              <span class="font-semibold text-gray-800 dark:text-slate-100">New File</span>
              <span class="text-[10px] text-gray-400 dark:text-slate-400 truncate">Create empty file</span>
            </div>
          </button>

          <button
            type="button"
            @click="$emit('newFolder'); closeMenu()"
            class="w-full flex items-center space-x-2.5 px-2.5 py-2 rounded-xl hover:bg-gray-100/90 dark:hover:bg-slate-800/80 transition text-left cursor-pointer group"
          >
            <div class="w-7 h-7 rounded-xl bg-amber-500/10 text-amber-600 dark:text-amber-400 flex items-center justify-center shrink-0 border border-amber-500/20 group-hover:scale-105 transition-transform">
              <FbIcon name="new-folder" size="14px" />
            </div>
            <div class="flex flex-col min-w-0">
              <span class="font-semibold text-gray-800 dark:text-slate-100">New Folder</span>
              <span class="text-[10px] text-gray-400 dark:text-slate-400 truncate">Create directory</span>
            </div>
          </button>

          <div class="my-1 border-t border-gray-100 dark:border-slate-800/80"></div>

          <button
            type="button"
            @click="$emit('upload'); closeMenu()"
            class="w-full flex items-center space-x-2.5 px-2.5 py-2 rounded-xl hover:bg-gray-100/90 dark:hover:bg-slate-800/80 transition text-left cursor-pointer group"
          >
            <div class="w-7 h-7 rounded-xl bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 flex items-center justify-center shrink-0 border border-emerald-500/20 group-hover:scale-105 transition-transform">
              <FbIcon name="upload" size="14px" />
            </div>
            <div class="flex flex-col min-w-0">
              <span class="font-semibold text-gray-800 dark:text-slate-100">Upload Files</span>
              <span class="text-[10px] text-gray-400 dark:text-slate-400 truncate">From local device</span>
            </div>
          </button>

          <div class="my-1 border-t border-gray-100 dark:border-slate-800/80"></div>

          <button
            type="button"
            @click="$emit('sync'); closeMenu()"
            class="w-full flex items-center space-x-2.5 px-2.5 py-2 rounded-xl hover:bg-gray-100/90 dark:hover:bg-slate-800/80 transition text-left cursor-pointer group"
          >
            <div class="w-7 h-7 rounded-xl bg-cyan-500/10 text-cyan-600 dark:text-cyan-400 flex items-center justify-center shrink-0 border border-cyan-500/20 group-hover:scale-105 transition-transform">
              <FbIcon name="refresh" size="14px" />
            </div>
            <div class="flex flex-col min-w-0">
              <span class="font-semibold text-gray-800 dark:text-slate-100">Sync Folder...</span>
              <span class="text-[10px] text-gray-400 dark:text-slate-400 truncate">Bidirectional sync</span>
            </div>
          </button>
        </div>
      </Transition>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import FbIcon from '../../../components/common/FbIcon.vue';

type SortFieldType = 'name' | 'size' | 'modified' | 'kind';
type SortOrderType = 'asc' | 'desc';

withDefaults(
  defineProps<{
    viewMode: 'list' | 'grid';
    showHidden: boolean;
    sortField: SortFieldType;
    sortOrder: SortOrderType;
    isRefreshing?: boolean;
    isDualPane?: boolean;
  }>(),
  {
    isRefreshing: false,
    isDualPane: false,
  }
);

defineEmits<{
  (e: 'update:viewMode', mode: 'list' | 'grid'): void;
  (e: 'update:showHidden', show: boolean): void;
  (e: 'update:sortField', field: SortFieldType): void;
  (e: 'update:sortOrder', order: SortOrderType): void;
  (e: 'newFile'): void;
  (e: 'newFolder'): void;
  (e: 'upload'): void;
  (e: 'sync'): void;
  (e: 'selectAll'): void;
  (e: 'editPath'): void;
  (e: 'copyPath'): void;
  (e: 'swapPanels'): void;
  (e: 'refresh'): void;
}>();

const sortMenuRef = ref<HTMLElement | null>(null);
const newMenuRef = ref<HTMLElement | null>(null);

type ActiveMenu = 'sort' | 'new' | null;
const activeMenu = ref<ActiveMenu>(null);

const isSortOpen = computed(() => activeMenu.value === 'sort');
const isNewOpen = computed(() => activeMenu.value === 'new');

function toggleSort() {
  activeMenu.value = activeMenu.value === 'sort' ? null : 'sort';
}

function toggleNew() {
  activeMenu.value = activeMenu.value === 'new' ? null : 'new';
}

function closeMenu() {
  activeMenu.value = null;
}

function handlePointerDownOutside(e: PointerEvent) {
  if (!activeMenu.value) return;
  const target = e.target as Node | null;
  if (
    target &&
    (sortMenuRef.value?.contains(target) || newMenuRef.value?.contains(target))
  ) {
    return;
  }
  closeMenu();
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape' && activeMenu.value) {
    e.stopPropagation();
    closeMenu();
  }
}

function handleScroll() {
  if (activeMenu.value) {
    closeMenu();
  }
}

onMounted(() => {
  window.addEventListener('pointerdown', handlePointerDownOutside);
  window.addEventListener('keydown', handleKeydown);
  window.addEventListener('scroll', handleScroll, true);
});

onUnmounted(() => {
  window.removeEventListener('pointerdown', handlePointerDownOutside);
  window.removeEventListener('keydown', handleKeydown);
  window.removeEventListener('scroll', handleScroll, true);
});

const sortFields: { id: SortFieldType; label: string }[] = [
  { id: 'name', label: 'Name' },
  { id: 'size', label: 'Size' },
  { id: 'modified', label: 'Last Modified' },
  { id: 'kind', label: 'Kind / Type' },
];
</script>

<template>
  <div class="flex items-center space-x-1.5 sm:space-x-2 shrink-0">
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
      class="p-1.5 rounded-xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 text-gray-600 dark:text-slate-400 hover:bg-gray-50 dark:hover:bg-slate-800 transition cursor-pointer active:scale-95 duration-fast ease-spring"
      title="Refresh Directory (F5)"
    >
      <FbIcon name="refresh" size="13px" :class="{ 'animate-spin': isRefreshing }" />
    </button>

    <!-- Sort & View Options Popover (···) -->
    <div ref="sortMenuRef" class="relative">
      <button
        type="button"
        @click="isSortOpen = !isSortOpen"
        :class="[
          'p-1.5 rounded-xl border transition cursor-pointer active:scale-95 duration-fast ease-spring flex items-center space-x-1',
          isSortOpen || showHidden || sortField !== 'name'
            ? 'text-blue-600 dark:text-blue-400 border-blue-500/40 bg-blue-50/50 dark:bg-blue-950/40 ring-1 ring-blue-500/20'
            : 'border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 text-gray-600 dark:text-slate-400 hover:bg-gray-50 dark:hover:bg-slate-800'
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
          class="absolute right-0 mt-2 w-56 sm:w-60 bg-white dark:bg-[#0f1422] border border-gray-200 dark:border-slate-800 rounded-2xl shadow-2xl p-2.5 z-50 text-xs text-gray-700 dark:text-slate-200 space-y-2.5"
        >
          <!-- SECTION 1: VIEW & VISIBILITY -->
          <div>
            <div class="px-2 py-0.5 text-[10px] font-bold text-gray-400 dark:text-slate-500 uppercase tracking-wider">
              VIEW & VISIBILITY
            </div>
            <div class="space-y-0.5 mt-1">
              <button
                type="button"
                @click="$emit('update:showHidden', !showHidden)"
                class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800/80 transition text-left cursor-pointer"
              >
                <div class="flex items-center space-x-2">
                  <FbIcon name="eye" size="13px" class="text-gray-400" />
                  <span>Show Hidden Dotfiles</span>
                </div>
                <span v-if="showHidden" class="text-blue-600 dark:text-blue-400 font-bold">✓</span>
              </button>

              <button
                type="button"
                @click="$emit('selectAll'); isSortOpen = false"
                class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800/80 transition text-left cursor-pointer"
              >
                <div class="flex items-center space-x-2">
                  <FbIcon name="select-all" size="13px" class="text-gray-400" />
                  <span>Select All Items</span>
                </div>
                <kbd class="text-[10px] text-gray-400 font-mono">⌘A</kbd>
              </button>
            </div>
          </div>

          <div class="border-t border-gray-100 dark:border-slate-800/80"></div>

          <!-- SECTION 2: SORT BY -->
          <div>
            <div class="px-2 py-0.5 text-[10px] font-bold text-gray-400 dark:text-slate-500 uppercase tracking-wider">
              SORT BY
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
                    : 'hover:bg-gray-100 dark:hover:bg-slate-800/80 text-gray-700 dark:text-slate-300'
                ]"
              >
                <span>{{ field.label }}</span>
                <span v-if="sortField === field.id" class="text-blue-600 dark:text-blue-400 font-bold">✓</span>
              </button>
            </div>
          </div>

          <!-- SECTION 3: ORDER (ASC / DESC) -->
          <div>
            <div class="grid grid-cols-2 gap-1 p-0.5 bg-gray-100 dark:bg-slate-800/80 rounded-xl">
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
              @click="$emit('editPath'); isSortOpen = false"
              class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800/80 transition text-left cursor-pointer"
            >
              <div class="flex items-center space-x-2">
                <FbIcon name="rename" size="13px" class="text-gray-400" />
                <span>Edit Path Directly</span>
              </div>
              <kbd class="text-[10px] text-gray-400 font-mono">⌘L</kbd>
            </button>
            <button
              type="button"
              @click="$emit('copyPath'); isSortOpen = false"
              class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800/80 transition text-left cursor-pointer"
            >
              <div class="flex items-center space-x-2">
                <FbIcon name="copy" size="13px" class="text-gray-400" />
                <span>Copy Current Path</span>
              </div>
            </button>
            <button
              v-if="isDualPane"
              type="button"
              @click="$emit('swapPanels'); isSortOpen = false"
              class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800/80 transition text-left cursor-pointer"
            >
              <div class="flex items-center space-x-2">
                <span class="text-sm">⇄</span>
                <span>Swap Panels</span>
              </div>
              <kbd class="text-[10px] text-gray-400 font-mono">Alt+S</kbd>
            </button>
          </div>
        </div>
      </Transition>
    </div>

    <!-- PRIMARY ACTION: + New Button (Filled Blue Pill) -->
    <div ref="newMenuRef" class="relative">
      <button
        type="button"
        @click="isNewOpen = !isNewOpen"
        class="bg-blue-600 hover:bg-blue-500 active:bg-blue-700 text-white font-semibold px-2.5 sm:px-3 py-1.5 rounded-xl flex items-center space-x-1 text-xs shadow-xs transition cursor-pointer active:scale-95 duration-fast ease-spring"
      >
        <FbIcon name="plus" size="13px" />
        <span class="hidden sm:inline">New</span>
      </button>

      <!-- New Item Dropdown Menu -->
      <Transition name="ios-popover">
        <div
          v-if="isNewOpen"
          @click="isNewOpen = false"
          class="absolute right-0 mt-2 w-44 bg-white dark:bg-[#0f1422] border border-gray-200 dark:border-slate-800 rounded-2xl shadow-xl p-1.5 z-50 text-xs text-gray-700 dark:text-slate-200 space-y-0.5"
        >
          <button
            type="button"
            @click="$emit('newFile')"
            class="w-full flex items-center space-x-2.5 px-3 py-2 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800 transition text-left cursor-pointer"
          >
            <FbIcon name="new-file" size="14px" class="text-blue-600" />
            <span class="font-medium">New File</span>
          </button>
          <button
            type="button"
            @click="$emit('newFolder')"
            class="w-full flex items-center space-x-2.5 px-3 py-2 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800 transition text-left cursor-pointer"
          >
            <FbIcon name="new-folder" size="14px" class="text-amber-500" />
            <span class="font-medium">New Folder</span>
          </button>
          <div class="my-1 border-t border-gray-100 dark:border-slate-800"></div>
          <button
            type="button"
            @click="$emit('upload')"
            class="w-full flex items-center space-x-2.5 px-3 py-2 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800 transition text-left cursor-pointer"
          >
            <FbIcon name="upload" size="14px" class="text-emerald-500" />
            <span class="font-medium">Upload Files</span>
          </button>
          <div class="my-1 border-t border-gray-100 dark:border-slate-800"></div>
          <button
            type="button"
            @click="$emit('sync')"
            class="w-full flex items-center space-x-2.5 px-3 py-2 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800 transition text-left cursor-pointer"
          >
            <span class="text-xs">🔄</span>
            <span class="font-medium">Sync Folder...</span>
          </button>
        </div>
      </Transition>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
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

const isSortOpen = ref(false);
const isNewOpen = ref(false);

const sortFields: { id: SortFieldType; label: string }[] = [
  { id: 'name', label: 'Name' },
  { id: 'size', label: 'Size' },
  { id: 'modified', label: 'Last Modified' },
  { id: 'kind', label: 'Kind / Type' },
];
</script>

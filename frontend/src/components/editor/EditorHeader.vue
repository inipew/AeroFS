<template>
  <div
    class="h-13 sm:h-14 bg-gray-50/95 dark:bg-[#0d121f]/95 border-b border-gray-200/80 dark:border-slate-800/80 px-3 sm:px-4 flex items-center justify-between text-xs shrink-0 gap-2 select-none"
  >
    <!-- Left: File Tab & Breadcrumb Path -->
    <div class="flex items-center space-x-2.5 truncate flex-1 min-w-0">
      <!-- Mobile Back Button -->
      <button
        v-if="isMobile"
        @click="$emit('close')"
        class="p-1.5 -ml-1 text-gray-500 hover:text-gray-900 dark:text-slate-400 dark:hover:text-white rounded-xl hover:bg-gray-200 dark:hover:bg-slate-800 transition cursor-pointer shrink-0"
        title="Close Editor"
      >
        <FbIcon name="chevron-left" size="18px" />
      </button>

      <!-- Category Icon Badge -->
      <div
        class="w-8 h-8 rounded-xl flex items-center justify-center shrink-0 border shadow-2xs"
        :class="[fileTypeMeta.badgeBg, fileTypeMeta.badgeBorder]"
      >
        <span class="text-sm select-none">{{ fileTypeMeta.symbol }}</span>
      </div>

      <!-- File Title & Path -->
      <div class="truncate flex-1 min-w-0">
        <div class="flex items-center space-x-2 truncate">
          <span class="font-bold text-gray-900 dark:text-white text-xs sm:text-sm truncate">
            {{ activeFile.name }}
          </span>
          <span
            v-if="isDirty"
            class="w-2 h-2 rounded-full bg-amber-500 animate-pulse shrink-0 ring-2 ring-amber-500/20"
            title="Unsaved changes (Ctrl+S to save)"
          ></span>
        </div>
        <div class="flex items-center space-x-1 text-[10px] text-gray-400 dark:text-slate-500 font-mono truncate">
          <span class="truncate">{{ activeFile.path }}</span>
          <button
            @click="copyPath"
            class="p-0.5 hover:text-blue-500 transition cursor-pointer shrink-0"
            title="Copy Path"
          >
            <FbIcon name="copy" size="10px" />
          </button>
        </div>
      </div>
    </div>

    <!-- Right Controls -->
    <div class="flex items-center space-x-1.5 sm:space-x-2 shrink-0">
      <!-- Find & Replace (Ctrl+F) -->
      <button
        @click="$emit('toggle-search')"
        :class="[
          'p-1.5 sm:px-2.5 sm:py-1.5 rounded-xl border text-xs font-semibold flex items-center space-x-1.5 transition cursor-pointer shadow-2xs',
          isSearchOpen
            ? 'bg-blue-50 dark:bg-blue-950/60 text-blue-600 dark:text-blue-400 border-blue-500/40'
            : 'bg-white dark:bg-slate-900 text-gray-700 dark:text-slate-300 border-gray-200/80 dark:border-slate-800 hover:bg-gray-50 dark:hover:bg-slate-800'
        ]"
        title="Find & Replace (Ctrl+F / Ctrl+H)"
      >
        <FbIcon name="search" size="13px" />
        <span class="hidden md:inline">Find</span>
      </button>

      <!-- Go to Line (Ctrl+G) -->
      <button
        @click="$emit('toggle-goto')"
        :class="[
          'p-1.5 sm:px-2.5 sm:py-1.5 rounded-xl border text-xs font-semibold flex items-center space-x-1.5 transition cursor-pointer shadow-2xs',
          isGotoOpen
            ? 'bg-blue-50 dark:bg-blue-950/60 text-blue-600 dark:text-blue-400 border-blue-500/40'
            : 'bg-white dark:bg-slate-900 text-gray-700 dark:text-slate-300 border-gray-200/80 dark:border-slate-800 hover:bg-gray-50 dark:hover:bg-slate-800'
        ]"
        title="Go to Line (Ctrl+G)"
      >
        <span class="font-mono text-xs font-bold leading-none">#</span>
        <span class="hidden md:inline">Go to</span>
      </button>

      <!-- Format Document (Shift+Alt+F) -->
      <button
        @click="$emit('format')"
        class="p-1.5 sm:px-2.5 sm:py-1.5 rounded-xl border bg-white dark:bg-slate-900 hover:bg-gray-50 dark:hover:bg-slate-800 border-gray-200/80 dark:border-slate-800 text-gray-700 dark:text-slate-300 text-xs font-semibold flex items-center space-x-1.5 transition cursor-pointer shadow-2xs"
        title="Format Code (Shift+Alt+F)"
      >
        <FbIcon name="code" size="13px" />
        <span class="hidden md:inline">Format</span>
      </button>

      <!-- Markdown Preview Toggle -->
      <button
        v-if="isMarkdown"
        @click="$emit('toggle-preview')"
        :class="[
          'p-1.5 sm:px-2.5 sm:py-1.5 rounded-xl border text-xs font-semibold flex items-center space-x-1.5 transition cursor-pointer shadow-2xs',
          showMarkdownPreview
            ? 'bg-blue-50 dark:bg-blue-950/60 text-blue-600 dark:text-blue-400 border-blue-500/40'
            : 'bg-white dark:bg-slate-900 text-gray-700 dark:text-slate-300 border-gray-200/80 dark:border-slate-800 hover:bg-gray-50'
        ]"
        title="Toggle Markdown Live Preview"
      >
        <FbIcon name="eye" size="13px" />
        <span class="hidden md:inline">{{ showMarkdownPreview ? 'Editor' : 'Preview' }}</span>
      </button>

      <!-- Settings Popover Button -->
      <button
        data-settings-trigger="true"
        @click="$emit('toggle-settings')"
        :class="[
          'p-1.5 sm:px-2.5 sm:py-1.5 rounded-xl border text-xs font-semibold flex items-center space-x-1.5 transition cursor-pointer shadow-2xs',
          isSettingsOpen
            ? 'bg-blue-50 dark:bg-blue-950/60 text-blue-600 dark:text-blue-400 border-blue-500/40'
            : 'bg-white dark:bg-slate-900 text-gray-700 dark:text-slate-300 border-gray-200/80 dark:border-slate-800 hover:bg-gray-50 dark:hover:bg-slate-800'
        ]"
        title="Editor Settings (Theme, Font, Tab)"
      >
        <FbIcon name="settings" size="13px" />
      </button>

      <!-- Save Button -->
      <button
        @click="$emit('save')"
        :disabled="saving || !isDirty"
        :class="[
          'px-3 sm:px-3.5 py-1.5 rounded-xl font-bold flex items-center space-x-1.5 transition shadow-xs cursor-pointer text-xs disabled:cursor-not-allowed',
          isDirty
            ? 'bg-blue-600 hover:bg-blue-700 active:bg-blue-800 text-white shadow-blue-500/20'
            : 'bg-gray-100 dark:bg-slate-800/60 text-gray-400 dark:text-slate-500 opacity-60 border border-gray-200 dark:border-slate-800'
        ]"
        title="Save Changes (Ctrl+S)"
      >
        <FbIcon name="save" size="13px" :class="{ 'animate-spin': saving }" />
        <span>{{ saving ? 'Saving...' : 'Save' }}</span>
      </button>

      <!-- Close Button -->
      <button
        v-if="!isMobile"
        @click="$emit('close')"
        class="p-1.5 sm:px-2.5 sm:py-1.5 bg-gray-100 dark:bg-slate-800/80 hover:bg-gray-200 dark:hover:bg-slate-700 text-gray-600 dark:text-slate-300 rounded-xl transition text-xs font-semibold cursor-pointer border border-transparent hover:border-gray-200 dark:hover:border-slate-700"
        title="Close Editor (Esc)"
      >
        ✕
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import type { FileEntry } from '../../types/vfs';
import FbIcon from '../common/FbIcon.vue';
import { getFileTypeMeta } from '../../utils/fileTypes';
import { useUiStore } from '../../stores/uiStore';

const props = defineProps<{
  activeFile: FileEntry;
  isDirty: boolean;
  saving: boolean;
  isMarkdown: boolean;
  showMarkdownPreview: boolean;
  isSettingsOpen: boolean;
  isSearchOpen: boolean;
  isGotoOpen: boolean;
  isMobile: boolean;
}>();

defineEmits<{
  (e: 'toggle-search'): void;
  (e: 'toggle-goto'): void;
  (e: 'format'): void;
  (e: 'toggle-preview'): void;
  (e: 'toggle-settings'): void;
  (e: 'save'): void;
  (e: 'close'): void;
}>();

const uiStore = useUiStore();

const fileTypeMeta = computed(() => {
  return getFileTypeMeta(props.activeFile);
});

function copyPath() {
  if (props.activeFile?.path) {
    navigator.clipboard.writeText(props.activeFile.path);
    uiStore.showToast('Path copied to clipboard', 'info');
  }
}
</script>

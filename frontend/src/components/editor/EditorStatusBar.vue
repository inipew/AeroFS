<template>
  <div
    class="h-7 md:h-8 bg-gray-50/95 dark:bg-[#0d121f]/95 border-t border-gray-200/80 dark:border-slate-800/80 px-3 md:px-4 flex items-center justify-between text-[10px] md:text-[11px] text-gray-500 dark:text-slate-400 select-none shrink-0 font-mono relative z-20"
  >
    <!-- Left: Line / Col & Metrics -->
    <div class="flex items-center space-x-2.5 sm:space-x-3.5 truncate">
      <span class="text-gray-700 dark:text-slate-200 font-semibold">
        Ln {{ cursorPosition.row + 1 }}, Col {{ cursorPosition.column + 1 }}
      </span>
      <span v-if="selectedCharCount > 0" class="text-blue-600 dark:text-blue-400 font-medium">
        ({{ selectedCharCount }} selected)
      </span>

      <span class="text-gray-300 dark:text-slate-700">•</span>
      <span>{{ lineCount }} lines</span>
      <span class="text-gray-300 dark:text-slate-700">•</span>
      <span>{{ charCount }} chars</span>

      <!-- Fixed Encoding Details -->
      <span class="hidden sm:inline text-gray-300 dark:text-slate-700">•</span>
      <span class="hidden sm:inline">UTF-8</span>
      <span class="hidden sm:inline">LF</span>
    </div>

    <!-- Right: Spacing, Language Mode Popover & Saved Indicator -->
    <div class="flex items-center space-x-2.5 shrink-0">
      <!-- Tab Spacing Selector Popover -->
      <div class="relative hidden sm:block">
        <button
          @click.stop="isTabMenuOpen = !isTabMenuOpen; isLangMenuOpen = false"
          class="hover:text-gray-800 dark:hover:text-slate-200 cursor-pointer flex items-center space-x-1"
          title="Change Indentation"
        >
          <span>Spaces: {{ tabSize }}</span>
        </button>

        <div
          v-if="isTabMenuOpen"
          class="absolute bottom-full mb-1.5 right-0 bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-xl shadow-xl py-1 w-28 z-50 animate-in fade-in zoom-in-95 duration-100"
          @click.stop
        >
          <button
            v-for="size in [2, 4, 8]"
            :key="size"
            @click="$emit('update-tab-size', size); isTabMenuOpen = false"
            :class="[
              'w-full px-3 py-1.5 text-left text-xs transition cursor-pointer flex items-center justify-between',
              tabSize === size
                ? 'bg-blue-600 text-white font-bold'
                : 'text-gray-700 dark:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800'
            ]"
          >
            <span>{{ size }} spaces</span>
            <span v-if="tabSize === size">✓</span>
          </button>
        </div>
      </div>

      <span class="hidden sm:inline text-gray-300 dark:text-slate-700">•</span>

      <!-- Language Mode Popover -->
      <div class="relative">
        <button
          @click.stop="isLangMenuOpen = !isLangMenuOpen; isTabMenuOpen = false"
          class="hover:text-gray-800 dark:hover:text-slate-200 cursor-pointer flex items-center space-x-1 text-blue-600 dark:text-blue-400 font-semibold"
          title="Select Language Mode"
        >
          <span class="max-w-[100px] truncate">{{ currentModeName }}</span>
          <FbIcon name="chevron-down" size="10px" />
        </button>

        <div
          v-if="isLangMenuOpen"
          class="absolute bottom-full mb-1.5 right-0 w-56 bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-2xl shadow-2xl p-2 z-50 animate-in fade-in zoom-in-95 duration-100"
          @click.stop
        >
          <input
            v-model="languageSearchQuery"
            type="text"
            placeholder="Search language..."
            class="w-full bg-gray-50 dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded-xl px-2.5 py-1.5 text-xs text-gray-900 dark:text-slate-100 outline-none mb-1.5 font-medium font-sans"
          />
          <div class="max-h-56 overflow-y-auto space-y-0.5 scrollbar-thin">
            <button
              v-for="lang in filteredLanguages"
              :key="lang.mode"
              @click="$emit('select-language', lang.mode); isLangMenuOpen = false"
              :class="[
                'w-full flex items-center justify-between px-2.5 py-1.5 rounded-xl text-left text-xs transition cursor-pointer font-sans',
                currentMode === lang.mode
                  ? 'bg-blue-600 text-white font-bold'
                  : 'text-gray-700 dark:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800'
              ]"
            >
              <span>{{ lang.name }}</span>
              <span v-if="currentMode === lang.mode" class="text-xs font-bold">✓</span>
            </button>
          </div>
        </div>
      </div>

      <span class="text-gray-300 dark:text-slate-700">•</span>

      <!-- Saved / Unsaved Indicator -->
      <div class="flex items-center space-x-1.5">
        <span v-if="isDirty" class="text-amber-500 font-bold flex items-center space-x-1">
          <span class="w-1.5 h-1.5 rounded-full bg-amber-500 animate-pulse"></span>
          <span>Unsaved</span>
        </span>
        <span v-else class="text-emerald-500 font-bold flex items-center space-x-1">
          <span class="w-1.5 h-1.5 rounded-full bg-emerald-500"></span>
          <span>Saved</span>
        </span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount } from 'vue';
import type { EditorCursorPosition } from '../../services/editorSession';
import FbIcon from '../common/FbIcon.vue';

const props = defineProps<{
  cursorPosition: EditorCursorPosition;
  selectedCharCount: number;
  lineCount: number;
  charCount: number;
  tabSize: number;
  currentMode: string;
  availableLanguages: Array<{ name: string; mode: string }>;
  isDirty: boolean;
}>();

defineEmits<{
  (e: 'update-tab-size', size: number): void;
  (e: 'select-language', mode: string): void;
}>();

const isTabMenuOpen = ref(false);
const isLangMenuOpen = ref(false);
const languageSearchQuery = ref('');

const currentModeName = computed(() => {
  const found = props.availableLanguages.find((l) => l.mode === props.currentMode);
  if (found) return found.name;
  const lang = props.currentMode.split('/').pop() || 'Text';
  return lang.charAt(0).toUpperCase() + lang.slice(1);
});

const filteredLanguages = computed(() => {
  if (!languageSearchQuery.value.trim()) return props.availableLanguages;
  const q = languageSearchQuery.value.toLowerCase();
  return props.availableLanguages.filter((l) => l.name.toLowerCase().includes(q));
});

function handleWindowClick() {
  isTabMenuOpen.value = false;
  isLangMenuOpen.value = false;
}

onMounted(() => {
  window.addEventListener('click', handleWindowClick);
});

onBeforeUnmount(() => {
  window.removeEventListener('click', handleWindowClick);
});
</script>

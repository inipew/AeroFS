<template>
  <div>
    <!-- Desktop Popover (top-right under toolbar) -->
    <div
      v-if="modelValue && !isMobile"
      ref="settingsCardRef"
      class="absolute top-14 right-3 z-40 bg-white/95 dark:bg-[#111827]/95 border border-gray-200 dark:border-slate-700 rounded-2xl shadow-2xl p-4 w-80 text-xs select-none backdrop-blur-md animate-in slide-in-from-top-2 duration-150 text-gray-800 dark:text-slate-200 space-y-3.5"
      @click.stop
    >
      <div class="flex items-center justify-between border-b border-gray-100 dark:border-slate-800 pb-2.5">
        <span class="font-bold text-gray-900 dark:text-white flex items-center space-x-1.5">
          <FbIcon name="settings" size="14px" class="text-blue-500" />
          <span>Editor Settings</span>
        </span>
        <button
          @click="$emit('update:modelValue', false)"
          class="p-1 hover:bg-gray-100 dark:hover:bg-slate-800 rounded-lg text-gray-400 hover:text-gray-700 dark:hover:text-white cursor-pointer"
        >
          ✕
        </button>
      </div>

      <!-- Theme Select -->
      <div class="space-y-1">
        <label class="font-semibold text-gray-500 dark:text-slate-400">Color Theme</label>
        <select
          :value="preferences.theme"
          @change="onThemeChange"
          class="w-full bg-gray-50 dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-xl px-2.5 py-1.5 text-gray-800 dark:text-slate-100 font-medium cursor-pointer shadow-2xs focus:outline-none"
        >
          <optgroup label="Dark Themes" class="bg-white dark:bg-slate-900">
            <option value="ace/theme/tomorrow_night">Tomorrow Night</option>
            <option value="ace/theme/one_dark">One Dark</option>
            <option value="ace/theme/dracula">Dracula</option>
            <option value="ace/theme/monokai">Monokai</option>
            <option value="ace/theme/nord_dark">Nord</option>
            <option value="ace/theme/twilight">Twilight</option>
            <option value="ace/theme/cobalt">Cobalt</option>
          </optgroup>
          <optgroup label="Light Themes" class="bg-white dark:bg-slate-900">
            <option value="ace/theme/chrome">Chrome</option>
            <option value="ace/theme/github">GitHub Light</option>
            <option value="ace/theme/tomorrow">Tomorrow Light</option>
            <option value="ace/theme/solarized_light">Solarized Light</option>
            <option value="ace/theme/textmate">TextMate</option>
          </optgroup>
        </select>
      </div>

      <!-- Font Family -->
      <div class="space-y-1">
        <label class="font-semibold text-gray-500 dark:text-slate-400">Font Family</label>
        <select
          :value="preferences.fontFamily"
          @change="onFontFamilyChange"
          class="w-full bg-gray-50 dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-xl px-2.5 py-1.5 text-gray-800 dark:text-slate-100 font-medium cursor-pointer shadow-2xs focus:outline-none font-mono"
        >
          <option value="'JetBrains Mono', 'Fira Code', monospace">JetBrains Mono</option>
          <option value="'Fira Code', monospace">Fira Code</option>
          <option value="'SF Mono', 'Menlo', monospace">SF Mono / Menlo</option>
          <option value="'Courier New', monospace">Courier New</option>
        </select>
      </div>

      <!-- Font Size Buttons -->
      <div class="space-y-1">
        <label class="font-semibold text-gray-500 dark:text-slate-400">Font Size</label>
        <div class="grid grid-cols-6 gap-1">
          <button
            v-for="s in [11, 12, 13, 14, 16, 18]"
            :key="s"
            @click="$emit('update-preference', 'fontSize', s)"
            :class="[
              'py-1 rounded-lg font-bold text-xs transition cursor-pointer text-center',
              preferences.fontSize === s
                ? 'bg-blue-600 text-white shadow-2xs'
                : 'bg-gray-100 dark:bg-slate-800 text-gray-700 dark:text-slate-300 hover:bg-gray-200 dark:hover:bg-slate-700'
            ]"
          >
            {{ s }}
          </button>
        </div>
      </div>

      <!-- Tab Size -->
      <div class="space-y-1">
        <label class="font-semibold text-gray-500 dark:text-slate-400">Tab Size</label>
        <div class="grid grid-cols-3 gap-1">
          <button
            v-for="ts in [2, 4, 8]"
            :key="ts"
            @click="$emit('update-preference', 'tabSize', ts)"
            :class="[
              'py-1 rounded-lg font-bold text-xs transition cursor-pointer text-center',
              preferences.tabSize === ts
                ? 'bg-blue-600 text-white shadow-2xs'
                : 'bg-gray-100 dark:bg-slate-800 text-gray-700 dark:text-slate-300 hover:bg-gray-200 dark:hover:bg-slate-700'
            ]"
          >
            {{ ts }} spaces
          </button>
        </div>
      </div>

      <!-- Toggles -->
      <div class="pt-1.5 space-y-2 border-t border-gray-100 dark:border-slate-800">
        <label class="flex items-center justify-between cursor-pointer">
          <span class="text-gray-700 dark:text-slate-300 font-medium">Word Wrap</span>
          <input
            type="checkbox"
            :checked="preferences.wordWrap"
            @change="$emit('update-preference', 'wordWrap', ($event.target as HTMLInputElement).checked)"
            class="rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
          />
        </label>
        <label class="flex items-center justify-between cursor-pointer">
          <span class="text-gray-700 dark:text-slate-300 font-medium">Line Numbers (Gutter)</span>
          <input
            type="checkbox"
            :checked="preferences.showGutter"
            @change="$emit('update-preference', 'showGutter', ($event.target as HTMLInputElement).checked)"
            class="rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
          />
        </label>
        <label class="flex items-center justify-between cursor-pointer">
          <span class="text-gray-700 dark:text-slate-300 font-medium">Highlight Active Line</span>
          <input
            type="checkbox"
            :checked="preferences.highlightActiveLine"
            @change="$emit('update-preference', 'highlightActiveLine', ($event.target as HTMLInputElement).checked)"
            class="rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
          />
        </label>
      </div>
    </div>

    <!-- Mobile Bottom Sheet (< 768px) -->
    <div
      v-if="modelValue && isMobile"
      class="fixed inset-0 z-50 bg-black/60 backdrop-blur-xs flex flex-col justify-end"
      @click="$emit('update:modelValue', false)"
    >
      <div
        class="bg-white dark:bg-[#0b0f19] border-t border-gray-200 dark:border-slate-800 rounded-t-3xl shadow-2xl p-5 pb-safe space-y-4 animate-in slide-in-from-bottom duration-200 max-h-[85vh] overflow-y-auto"
        @click.stop
      >
        <div class="w-12 h-1 bg-gray-300 dark:bg-slate-700 rounded-full mx-auto -mt-2 mb-2"></div>
        <div class="flex items-center justify-between border-b border-gray-100 dark:border-slate-800 pb-3">
          <h3 class="font-bold text-sm text-gray-900 dark:text-white flex items-center space-x-2">
            <FbIcon name="settings" size="16px" class="text-blue-600 dark:text-blue-400" />
            <span>Editor Settings</span>
          </h3>
          <button
            @click="$emit('update:modelValue', false)"
            class="p-1 text-gray-400 hover:text-gray-700 dark:hover:text-white text-base cursor-pointer"
          >
            ✕
          </button>
        </div>

        <div class="space-y-1.5">
          <label class="font-semibold text-xs text-gray-700 dark:text-slate-300">Color Theme</label>
          <select
            :value="preferences.theme"
            @change="onThemeChange"
            class="w-full bg-gray-50 dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-2xl px-3.5 py-2 text-gray-800 dark:text-slate-100 font-medium text-xs cursor-pointer shadow-2xs"
          >
            <optgroup label="Dark Themes" class="bg-white dark:bg-slate-900">
              <option value="ace/theme/tomorrow_night">Tomorrow Night</option>
              <option value="ace/theme/one_dark">One Dark</option>
              <option value="ace/theme/dracula">Dracula</option>
              <option value="ace/theme/monokai">Monokai</option>
              <option value="ace/theme/nord_dark">Nord</option>
            </optgroup>
            <optgroup label="Light Themes" class="bg-white dark:bg-slate-900">
              <option value="ace/theme/chrome">Chrome</option>
              <option value="ace/theme/github">GitHub Light</option>
              <option value="ace/theme/tomorrow">Tomorrow Light</option>
            </optgroup>
          </select>
        </div>

        <div class="space-y-1.5">
          <label class="font-semibold text-xs text-gray-700 dark:text-slate-300">Font Size</label>
          <div class="grid grid-cols-5 gap-1.5">
            <button
              v-for="s in [11, 12, 13, 14, 16]"
              :key="s"
              @click="$emit('update-preference', 'fontSize', s)"
              :class="[
                'py-2 rounded-xl font-bold text-xs transition cursor-pointer text-center',
                preferences.fontSize === s
                  ? 'bg-blue-600 text-white shadow-2xs'
                  : 'bg-gray-100 dark:bg-slate-900 text-gray-700 dark:text-slate-300'
              ]"
            >
              {{ s }}px
            </button>
          </div>
        </div>

        <div class="grid grid-cols-2 gap-3 pt-2">
          <label class="flex items-center space-x-2.5 p-3 rounded-2xl bg-gray-50 dark:bg-slate-900 border border-gray-200 dark:border-slate-800 cursor-pointer">
            <input
              type="checkbox"
              :checked="preferences.wordWrap"
              @change="$emit('update-preference', 'wordWrap', ($event.target as HTMLInputElement).checked)"
              class="rounded w-4 h-4 text-blue-600 focus:ring-0 cursor-pointer"
            />
            <span class="font-semibold text-xs text-gray-800 dark:text-slate-200">Word Wrap</span>
          </label>

          <label class="flex items-center space-x-2.5 p-3 rounded-2xl bg-gray-50 dark:bg-slate-900 border border-gray-200 dark:border-slate-800 cursor-pointer">
            <input
              type="checkbox"
              :checked="preferences.showGutter"
              @change="$emit('update-preference', 'showGutter', ($event.target as HTMLInputElement).checked)"
              class="rounded w-4 h-4 text-blue-600 focus:ring-0 cursor-pointer"
            />
            <span class="font-semibold text-xs text-gray-800 dark:text-slate-200">Line Numbers</span>
          </label>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from 'vue';
import type { EditorPreferences } from '../../services/editorSession';
import FbIcon from '../common/FbIcon.vue';

const props = defineProps<{
  modelValue: boolean;
  preferences: EditorPreferences;
  isMobile: boolean;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', val: boolean): void;
  (e: 'update-preference', key: keyof EditorPreferences, val: any): void;
}>();

const settingsCardRef = ref<HTMLElement | null>(null);

function onThemeChange(e: Event) {
  const val = (e.target as HTMLSelectElement).value;
  emit('update-preference', 'theme', val);
}

function onFontFamilyChange(e: Event) {
  const val = (e.target as HTMLSelectElement).value;
  emit('update-preference', 'fontFamily', val);
}

function handleDocumentClick(e: MouseEvent) {
  if (!props.modelValue || props.isMobile) return;
  const target = e.target as HTMLElement;
  if (settingsCardRef.value && !settingsCardRef.value.contains(target)) {
    if (target.closest('[data-settings-trigger]')) return;
    emit('update:modelValue', false);
  }
}

onMounted(() => {
  window.addEventListener('click', handleDocumentClick);
});

onBeforeUnmount(() => {
  window.removeEventListener('click', handleDocumentClick);
});
</script>

<template>
  <div
    v-if="uiStore.isEditorOpen && uiStore.editorFile"
    class="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex flex-col p-0 md:p-4 select-none font-sans text-xs animate-in fade-in duration-150"
    @click="closeAllDropdowns"
  >
    <div
      class="bg-white dark:bg-[#0b0f19] border-0 md:border md:border-gray-200 dark:md:border-slate-800 rounded-none md:rounded-3xl flex-1 flex flex-col shadow-2xl overflow-hidden relative"
      @click.stop
    >
      <!-- MOBILE HEADER (< 768px) -->
      <div
        v-if="uiStore.isMobile"
        class="h-14 bg-gray-50 dark:bg-[#090d16] border-b border-gray-200 dark:border-slate-800 px-3 flex items-center justify-between text-xs shrink-0 gap-2 select-none"
      >
        <!-- Left: Back Button + File Title -->
        <div class="flex items-center space-x-2 truncate flex-1 min-w-0">
          <button
            @click="handleClose"
            class="p-1.5 -ml-1 text-gray-600 hover:text-gray-900 dark:text-slate-400 dark:hover:text-white rounded-xl hover:bg-gray-200 dark:hover:bg-slate-800 transition cursor-pointer shrink-0"
            title="Close Editor"
          >
            <FbIcon name="chevron-left" size="20px" />
          </button>
          <div class="truncate flex-1 min-w-0">
            <div class="flex items-center space-x-1.5 truncate">
              <span class="font-bold text-gray-900 dark:text-white text-xs truncate">{{ uiStore.editorFile.name }}</span>
              <span v-if="isDirty" class="w-2 h-2 rounded-full bg-amber-500 animate-pulse shrink-0" title="Unsaved changes"></span>
              <span class="px-1.5 py-0.5 rounded bg-blue-50 dark:bg-blue-900/40 text-blue-700 dark:text-blue-300 font-mono text-[9px] font-semibold uppercase border border-blue-200 dark:border-blue-800/50 shrink-0">
                {{ currentModeName }}
              </span>
            </div>
            <p class="text-[10px] text-gray-400 dark:text-slate-500 font-mono truncate">{{ uiStore.editorFile.path }}</p>
          </div>
        </div>

        <!-- Right: Markdown Preview Toggle, Settings Button, Save Button -->
        <div class="flex items-center space-x-2 shrink-0">
          <!-- Markdown Preview Toggle (Mobile Tab) -->
          <button
            v-if="isMarkdownFile"
            @click="showMarkdownPreview = !showMarkdownPreview"
            :class="[
              'p-2 rounded-xl border transition cursor-pointer',
              showMarkdownPreview
                ? 'bg-blue-50 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400 border-blue-500/40'
                : 'bg-white dark:bg-slate-900 text-gray-600 dark:text-slate-400 border-gray-200 dark:border-slate-800'
            ]"
            :title="showMarkdownPreview ? 'Show Code' : 'Show Markdown Preview'"
          >
            <FbIcon name="eye" size="16px" />
          </button>

          <!-- Settings Button (Toggles Bottom Sheet on Mobile) -->
          <button
            @click.stop="isSettingsOpen = !isSettingsOpen"
            :class="[
              'p-2 rounded-xl border transition cursor-pointer',
              isSettingsOpen
                ? 'bg-blue-50 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400 border-blue-500/40'
                : 'bg-white dark:bg-slate-900 text-gray-600 dark:text-slate-400 border-gray-200 dark:border-slate-800 hover:bg-gray-50'
            ]"
            title="Editor Settings"
          >
            <FbIcon name="settings" size="16px" />
          </button>

          <!-- Save Button -->
          <button
            @click="handleSave"
            :disabled="saving || !isDirty"
            class="px-3.5 py-2 bg-blue-600 hover:bg-blue-700 active:bg-blue-800 disabled:opacity-40 text-white font-bold rounded-xl flex items-center space-x-1.5 transition shadow-xs cursor-pointer text-xs"
          >
            <FbIcon name="save" size="14px" />
            <span>{{ saving ? '...' : 'Save' }}</span>
          </button>
        </div>
      </div>

      <!-- DESKTOP HEADER (≥ 768px) -->
      <div
        v-else
        class="h-14 bg-gray-50 dark:bg-[#090d16] border-b border-gray-200 dark:border-slate-800 px-4 flex items-center justify-between text-xs shrink-0 gap-3"
      >
        <!-- Left: File Badge & Title -->
        <div class="flex items-center space-x-3 truncate">
          <div class="w-9 h-9 rounded-xl bg-blue-600/10 dark:bg-blue-500/20 text-blue-600 dark:text-blue-400 flex items-center justify-center shrink-0">
            <FbIcon name="file" size="18px" />
          </div>
          <div class="truncate">
            <div class="flex items-center space-x-2">
              <span class="font-bold text-gray-900 dark:text-white text-sm truncate">{{ uiStore.editorFile.name }}</span>
              <span v-if="isDirty" class="w-2.5 h-2.5 rounded-full bg-amber-500 animate-pulse" title="Unsaved changes"></span>
              <span class="px-2 py-0.5 rounded-md bg-blue-50 dark:bg-blue-900/40 text-blue-700 dark:text-blue-300 font-mono text-[10px] font-semibold uppercase border border-blue-200 dark:border-blue-800/50">
                {{ currentModeName }}
              </span>
            </div>
            <p class="text-[11px] text-gray-400 dark:text-slate-500 font-mono truncate">{{ uiStore.editorFile.path }}</p>
          </div>
        </div>

        <!-- Center: Custom Syntax Selector, Markdown Toggle, Preferences Toggle -->
        <div class="flex items-center space-x-2 shrink-0">
          <!-- 1. Custom Syntax / Language Dropdown Menu -->
          <div class="relative">
            <button
              @click.stop="isSyntaxMenuOpen = !isSyntaxMenuOpen; isSettingsOpen = false"
              class="bg-white dark:bg-slate-900 hover:bg-gray-50 dark:hover:bg-slate-800 border border-gray-200 dark:border-slate-700/80 px-3 py-1.5 rounded-xl text-gray-700 dark:text-slate-200 flex items-center space-x-2 text-xs font-semibold shadow-xs transition cursor-pointer"
            >
              <span class="text-gray-400 dark:text-slate-400 font-normal">Syntax:</span>
              <span class="text-blue-600 dark:text-blue-400 font-bold">{{ currentModeName }}</span>
              <FbIcon name="chevron-down" size="12px" class="text-gray-400" />
            </button>

            <!-- Syntax Popover Menu -->
            <div
              v-if="isSyntaxMenuOpen"
              class="absolute top-full mt-1.5 left-0 w-48 bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-2xl shadow-2xl p-1.5 z-50 max-h-72 overflow-y-auto space-y-0.5 animate-in fade-in zoom-in-95 duration-100"
            >
              <button
                v-for="lang in availableLanguages"
                :key="lang.mode"
                @click="selectLanguage(lang)"
                :class="[
                  'w-full flex items-center justify-between px-3 py-1.5 rounded-xl text-left text-xs transition cursor-pointer',
                  currentMode === lang.mode
                    ? 'bg-blue-600 text-white font-bold'
                    : 'text-gray-700 dark:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800'
                ]"
              >
                <span>{{ lang.name }}</span>
                <span v-if="currentMode === lang.mode" class="text-xs">✓</span>
              </button>
            </div>
          </div>

          <!-- 2. Markdown Preview Split Toggle -->
          <button
            v-if="isMarkdownFile"
            @click="showMarkdownPreview = !showMarkdownPreview"
            :class="[
              'px-3 py-1.5 rounded-xl border font-semibold flex items-center space-x-1.5 transition shadow-xs cursor-pointer',
              showMarkdownPreview
                ? 'bg-blue-50 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400 border-blue-500/40'
                : 'bg-white dark:bg-slate-900 text-gray-700 dark:text-slate-300 border-gray-200 dark:border-slate-800 hover:bg-gray-50'
            ]"
            title="Toggle Markdown Live Preview"
          >
            <FbIcon name="eye" size="14px" />
            <span>{{ showMarkdownPreview ? 'Editor Only' : 'Split Preview' }}</span>
          </button>

          <!-- 3. Preferences Toggle Button -->
          <button
            @click.stop="isSettingsOpen = !isSettingsOpen; isSyntaxMenuOpen = false"
            :class="[
              'px-3 py-1.5 rounded-xl border font-semibold flex items-center space-x-1.5 transition shadow-xs cursor-pointer',
              isSettingsOpen
                ? 'bg-blue-50 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400 border-blue-500/40'
                : 'bg-white dark:bg-slate-900 text-gray-700 dark:text-slate-300 border-gray-200 dark:border-slate-700/80 hover:bg-gray-50 dark:hover:bg-slate-800'
            ]"
            title="Editor Appearance & Font Settings"
          >
            <FbIcon name="settings" size="14px" />
            <span>Preferences</span>
          </button>
        </div>

        <!-- Right Actions: Save & Close -->
        <div class="flex items-center space-x-2 shrink-0">
          <button
            @click="handleSave"
            :disabled="saving || !isDirty"
            class="px-4 py-2 bg-blue-600 hover:bg-blue-700 active:bg-blue-800 disabled:opacity-40 text-white font-semibold rounded-xl flex items-center space-x-1.5 transition shadow-xs cursor-pointer"
          >
            <FbIcon name="save" size="14px" />
            <span>{{ saving ? 'Saving...' : 'Save (Ctrl+S)' }}</span>
          </button>
          <button
            @click="handleClose"
            class="px-3.5 py-2 bg-gray-200 dark:bg-slate-800 hover:bg-gray-300 dark:hover:bg-slate-700 text-gray-700 dark:text-slate-200 rounded-xl transition text-xs font-semibold cursor-pointer"
          >
            Close
          </button>
        </div>
      </div>

      <!-- DESKTOP PREFERENCES DRAWER (≥ 768px) -->
      <div
        v-if="isSettingsOpen && !uiStore.isMobile"
        class="bg-gray-100 dark:bg-[#0f172a] border-b border-gray-200 dark:border-slate-800 px-4 py-2.5 flex flex-wrap items-center gap-4 text-xs select-none animate-in slide-in-from-top-2 duration-150 text-gray-800 dark:text-slate-200"
      >
        <!-- Theme -->
        <div class="flex items-center space-x-1.5">
          <span class="text-gray-500 dark:text-slate-400 font-medium">Theme:</span>
          <select
            v-model="editorTheme"
            @change="updateEditorTheme"
            class="bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-xl px-2.5 py-1 text-gray-800 dark:text-slate-100 font-medium cursor-pointer shadow-xs focus:outline-none"
          >
            <optgroup label="Dark Themes" class="bg-white dark:bg-slate-900 text-gray-800 dark:text-slate-100">
              <option value="ace/theme/tomorrow_night">Tomorrow Night</option>
              <option value="ace/theme/monokai">Monokai</option>
              <option value="ace/theme/dracula">Dracula</option>
              <option value="ace/theme/one_dark">One Dark</option>
              <option value="ace/theme/nord_dark">Nord</option>
              <option value="ace/theme/twilight">Twilight</option>
              <option value="ace/theme/cobalt">Cobalt</option>
            </optgroup>
            <optgroup label="Light Themes" class="bg-white dark:bg-slate-900 text-gray-800 dark:text-slate-100">
              <option value="ace/theme/chrome">Chrome</option>
              <option value="ace/theme/github">GitHub Light</option>
              <option value="ace/theme/tomorrow">Tomorrow Light</option>
              <option value="ace/theme/solarized_light">Solarized Light</option>
              <option value="ace/theme/textmate">TextMate</option>
            </optgroup>
          </select>
        </div>

        <!-- Font Size -->
        <div class="flex items-center space-x-1.5">
          <span class="text-gray-500 dark:text-slate-400 font-medium">Size:</span>
          <select
            v-model="fontSize"
            @change="updateEditorFont"
            class="bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-xl px-2.5 py-1 text-gray-800 dark:text-slate-100 font-medium cursor-pointer shadow-xs focus:outline-none"
          >
            <option :value="11">11px</option>
            <option :value="12">12px</option>
            <option :value="13">13px</option>
            <option :value="14">14px</option>
            <option :value="16">16px</option>
            <option :value="18">18px</option>
          </select>
        </div>

        <!-- Tab Size -->
        <div class="flex items-center space-x-1.5">
          <span class="text-gray-500 dark:text-slate-400 font-medium">Tab:</span>
          <select
            v-model="tabSize"
            @change="updateEditorTabSize"
            class="bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-xl px-2.5 py-1 text-gray-800 dark:text-slate-100 font-medium cursor-pointer shadow-xs focus:outline-none"
          >
            <option :value="2">2 spaces</option>
            <option :value="4">4 spaces</option>
          </select>
        </div>

        <!-- Word Wrap Toggle -->
        <label class="flex items-center space-x-1.5 cursor-pointer">
          <input
            type="checkbox"
            v-model="wordWrap"
            @change="updateEditorWrap"
            class="rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
          />
          <span class="text-gray-700 dark:text-slate-300 font-medium">Wrap</span>
        </label>

        <!-- Line Numbers Toggle -->
        <label class="flex items-center space-x-1.5 cursor-pointer">
          <input
            type="checkbox"
            v-model="showGutter"
            @change="updateEditorGutter"
            class="rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
          />
          <span class="text-gray-700 dark:text-slate-300 font-medium">Lines</span>
        </label>
      </div>

      <!-- Main Editor Body & Markdown Preview -->
      <div class="flex-1 flex overflow-hidden relative bg-white dark:bg-[#0b0f19]">
        <!-- Ace Code Editor Mount Point -->
        <div
          v-show="!uiStore.isMobile || !showMarkdownPreview"
          ref="editorEl"
          :class="[
            'h-full text-xs sm:text-sm font-mono transition-all',
            !uiStore.isMobile && showMarkdownPreview ? 'w-1/2 border-r border-gray-200 dark:border-slate-800' : 'w-full'
          ]"
        ></div>

        <!-- Markdown Live Render Preview -->
        <div
          v-if="showMarkdownPreview"
          :class="[
            'h-full overflow-y-auto p-4 sm:p-6 bg-white dark:bg-slate-900 text-gray-900 dark:text-slate-100 prose dark:prose-invert max-w-none text-xs',
            uiStore.isMobile ? 'w-full' : 'w-1/2'
          ]"
          v-html="renderedMarkdown"
        ></div>
      </div>

      <!-- Mobile Virtual Keyboard Accessory Bar (Quick Syntax Helpers) -->
      <div
        v-if="uiStore.isMobile"
        class="h-11 bg-gray-100 dark:bg-slate-900 border-t border-gray-200 dark:border-slate-800 flex items-center px-2 space-x-1.5 overflow-x-auto shrink-0 select-none pb-safe"
      >
        <button
          v-for="sym in quickSymbols"
          :key="sym.label"
          @click="insertSymbol(sym.value)"
          class="px-3 py-1.5 bg-white dark:bg-slate-800 hover:bg-gray-200 dark:hover:bg-slate-700 text-gray-800 dark:text-slate-100 font-mono text-xs font-semibold rounded-lg shadow-xs border border-gray-200 dark:border-slate-700 shrink-0 cursor-pointer active:scale-95 transition"
        >
          {{ sym.label }}
        </button>
      </div>

      <!-- Editor Footer Status Bar -->
      <div class="h-7 md:h-8 bg-gray-50 dark:bg-[#090d16] border-t border-gray-200 dark:border-slate-800 px-3 md:px-4 flex items-center justify-between text-[10px] md:text-[11px] text-gray-500 dark:text-slate-500 select-none shrink-0 font-mono">
        <div class="flex items-center space-x-3">
          <span v-if="!uiStore.isMobile">Ace Editor v{{ aceVersion }}</span>
          <span>UTF-8</span>
          <span v-if="!uiStore.isMobile">Tab: {{ tabSize }} spaces</span>
          <span>{{ lineCount }}L / {{ charCount }}C</span>
        </div>
        <div class="flex items-center space-x-2">
          <span v-if="isDirty" class="text-amber-500 font-semibold flex items-center space-x-1">
            <span>●</span>
            <span>Unsaved</span>
          </span>
          <span v-else class="text-emerald-500 font-semibold flex items-center space-x-1">
            <span>●</span>
            <span>Saved</span>
          </span>
        </div>
      </div>

      <!-- MOBILE SETTINGS BOTTOM SHEET MODAL (< 768px) -->
      <div
        v-if="isSettingsOpen && uiStore.isMobile"
        class="fixed inset-0 z-50 bg-black/60 backdrop-blur-xs flex flex-col justify-end"
        @click="isSettingsOpen = false"
      >
        <div
          class="bg-white dark:bg-[#0b0f19] border-t border-gray-200 dark:border-slate-800 rounded-t-3xl shadow-2xl p-5 pb-safe space-y-4 animate-in slide-in-from-bottom duration-200 max-h-[80vh] overflow-y-auto"
          @click.stop
        >
          <!-- Drag Handle -->
          <div class="w-12 h-1 bg-gray-300 dark:bg-slate-700 rounded-full mx-auto -mt-2 mb-2"></div>

          <!-- Sheet Header -->
          <div class="flex items-center justify-between border-b border-gray-100 dark:border-slate-800 pb-3">
            <h3 class="font-bold text-sm text-gray-900 dark:text-white flex items-center space-x-2">
              <FbIcon name="settings" size="16px" class="text-blue-600 dark:text-blue-400" />
              <span>Editor Preferences</span>
            </h3>
            <button
              @click="isSettingsOpen = false"
              class="p-1 text-gray-400 hover:text-gray-700 dark:hover:text-white text-base"
            >
              ✕
            </button>
          </div>

          <!-- 1. Syntax Language Selection -->
          <div class="space-y-1.5">
            <label class="font-semibold text-xs text-gray-700 dark:text-slate-300">Syntax Language</label>
            <select
              v-model="currentMode"
              @change="updateLanguageFromSelect"
              class="w-full bg-gray-50 dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-2xl px-3.5 py-2.5 text-gray-800 dark:text-slate-100 font-medium text-xs cursor-pointer shadow-xs focus:outline-none"
            >
              <option v-for="lang in availableLanguages" :key="lang.mode" :value="lang.mode">
                {{ lang.name }}
              </option>
            </select>
          </div>

          <!-- 2. Editor Theme -->
          <div class="space-y-1.5">
            <label class="font-semibold text-xs text-gray-700 dark:text-slate-300">Color Theme</label>
            <select
              v-model="editorTheme"
              @change="updateEditorTheme"
              class="w-full bg-gray-50 dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-2xl px-3.5 py-2.5 text-gray-800 dark:text-slate-100 font-medium text-xs cursor-pointer shadow-xs focus:outline-none"
            >
              <optgroup label="Dark Themes" class="bg-white dark:bg-slate-900 text-gray-800 dark:text-slate-100">
                <option value="ace/theme/tomorrow_night">Tomorrow Night</option>
                <option value="ace/theme/monokai">Monokai</option>
                <option value="ace/theme/dracula">Dracula</option>
                <option value="ace/theme/one_dark">One Dark</option>
                <option value="ace/theme/nord_dark">Nord</option>
                <option value="ace/theme/twilight">Twilight</option>
                <option value="ace/theme/cobalt">Cobalt</option>
              </optgroup>
              <optgroup label="Light Themes" class="bg-white dark:bg-slate-900 text-gray-800 dark:text-slate-100">
                <option value="ace/theme/chrome">Chrome</option>
                <option value="ace/theme/github">GitHub Light</option>
                <option value="ace/theme/tomorrow">Tomorrow Light</option>
                <option value="ace/theme/solarized_light">Solarized Light</option>
                <option value="ace/theme/textmate">TextMate</option>
              </optgroup>
            </select>
          </div>

          <!-- 3. Font Size (Segmented Buttons) -->
          <div class="space-y-1.5">
            <label class="font-semibold text-xs text-gray-700 dark:text-slate-300">Font Size</label>
            <div class="grid grid-cols-5 gap-1.5">
              <button
                v-for="s in [11, 12, 13, 14, 16]"
                :key="s"
                @click="fontSize = s; updateEditorFont()"
                :class="[
                  'py-2 rounded-xl font-bold text-xs transition cursor-pointer text-center',
                  fontSize === s
                    ? 'bg-blue-600 text-white shadow-xs'
                    : 'bg-gray-100 dark:bg-slate-900 text-gray-700 dark:text-slate-300 hover:bg-gray-200 dark:hover:bg-slate-800'
                ]"
              >
                {{ s }}px
              </button>
            </div>
          </div>

          <!-- 4. Word Wrap & Line Numbers Toggles -->
          <div class="grid grid-cols-2 gap-3 pt-2">
            <label class="flex items-center space-x-2.5 p-3 rounded-2xl bg-gray-50 dark:bg-slate-900 border border-gray-200 dark:border-slate-800 cursor-pointer">
              <input
                type="checkbox"
                v-model="wordWrap"
                @change="updateEditorWrap"
                class="rounded w-4 h-4 text-blue-600 focus:ring-0 cursor-pointer"
              />
              <span class="font-semibold text-xs text-gray-800 dark:text-slate-200">Word Wrap</span>
            </label>

            <label class="flex items-center space-x-2.5 p-3 rounded-2xl bg-gray-50 dark:bg-slate-900 border border-gray-200 dark:border-slate-800 cursor-pointer">
              <input
                type="checkbox"
                v-model="showGutter"
                @change="updateEditorGutter"
                class="rounded w-4 h-4 text-blue-600 focus:ring-0 cursor-pointer"
              />
              <span class="font-semibold text-xs text-gray-800 dark:text-slate-200">Line Numbers</span>
            </label>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onBeforeUnmount } from 'vue';
import ace, { version as aceVersion } from 'ace-builds';
import modelist from 'ace-builds/src-noconflict/ext-modelist';
import FbIcon from '../common/FbIcon.vue';
import { apiClient } from '../../api/client';
import { useUiStore } from '../../stores/uiStore';
import { useFileStore } from '../../stores/fileStore';
import { useThemeStore } from '../../stores/themeStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';

// Point Ace to CDN scripts for dynamic mode / theme resolution
ace.config.set(
  'basePath',
  `https://cdn.jsdelivr.net/npm/ace-builds@${aceVersion}/src-min-noconflict/`
);

const uiStore = useUiStore();
const fileStore = useFileStore();
const themeStore = useThemeStore();
const workspaceStore = useWorkspaceStore();

const editorEl = ref<HTMLElement | null>(null);
const editor = ref<ace.Ace.Editor | null>(null);

const isDirty = ref(false);
const saving = ref(false);
const isSettingsOpen = ref(false);
const isSyntaxMenuOpen = ref(false);
const lineCount = ref(1);
const charCount = ref(0);
const showMarkdownPreview = ref(false);
const rawTextContent = ref('');

// Configurable Preferences (Loaded from localStorage)
const editorTheme = ref(localStorage.getItem('fb:editor:theme') || (themeStore.isDark ? 'ace/theme/tomorrow_night' : 'ace/theme/chrome'));
const fontFamily = ref(localStorage.getItem('fb:editor:fontFamily') || "'SF Mono', 'Menlo', monospace");
const fontSize = ref(Number(localStorage.getItem('fb:editor:fontSize')) || 13);
const tabSize = ref(Number(localStorage.getItem('fb:editor:tabSize')) || 2);
const wordWrap = ref(localStorage.getItem('fb:editor:wordWrap') !== 'false');
const showGutter = ref(localStorage.getItem('fb:editor:showGutter') !== 'false');
const currentMode = ref('ace/mode/text');

// Available programming languages for syntax selector
const availableLanguages = [
  { name: 'Rust', mode: 'ace/mode/rust' },
  { name: 'TypeScript', mode: 'ace/mode/typescript' },
  { name: 'JavaScript', mode: 'ace/mode/javascript' },
  { name: 'Python', mode: 'ace/mode/python' },
  { name: 'Go', mode: 'ace/mode/golang' },
  { name: 'HTML', mode: 'ace/mode/html' },
  { name: 'CSS', mode: 'ace/mode/css' },
  { name: 'JSON', mode: 'ace/mode/json' },
  { name: 'YAML', mode: 'ace/mode/yaml' },
  { name: 'Markdown', mode: 'ace/mode/markdown' },
  { name: 'SQL', mode: 'ace/mode/sql' },
  { name: 'Shell / Bash', mode: 'ace/mode/sh' },
  { name: 'PHP', mode: 'ace/mode/php' },
  { name: 'C / C++', mode: 'ace/mode/c_cpp' },
  { name: 'Java', mode: 'ace/mode/java' },
  { name: 'Dockerfile', mode: 'ace/mode/dockerfile' },
  { name: 'TOML', mode: 'ace/mode/toml' },
  { name: 'XML', mode: 'ace/mode/xml' },
  { name: 'Plain Text', mode: 'ace/mode/text' },
];

const quickSymbols = [
  { label: 'Tab', value: '\t' },
  { label: '{', value: '{' },
  { label: '}', value: '}' },
  { label: '(', value: '(' },
  { label: ')', value: ')' },
  { label: '[', value: '[' },
  { label: ']', value: ']' },
  { label: '=', value: '=' },
  { label: '/', value: '/' },
  { label: ':', value: ':' },
  { label: '"', value: '"' },
  { label: "'", value: "'" },
  { label: '`', value: '`' },
  { label: '_', value: '_' },
  { label: '$', value: '$' },
  { label: ';', value: ';' },
  { label: '<', value: '<' },
  { label: '>', value: '>' },
  { label: '!', value: '!' },
];

function insertSymbol(val: string) {
  if (editor.value) {
    editor.value.insert(val);
    editor.value.focus();
  }
}

const currentModeName = computed(() => {
  const found = availableLanguages.find((l) => l.mode === currentMode.value);
  if (found) return found.name;
  const lang = currentMode.value.split('/').pop() || 'Text';
  return lang.charAt(0).toUpperCase() + lang.slice(1);
});

const isMarkdownFile = computed(() => {
  if (!uiStore.editorFile) return false;
  const ext = uiStore.editorFile.name.split('.').pop()?.toLowerCase() || '';
  return ['md', 'markdown', 'mdown'].includes(ext);
});

function escapeHtml(str: string): string {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

// Simple safe markdown parser with strict HTML entity encoding to prevent XSS
const renderedMarkdown = computed(() => {
  if (!rawTextContent.value) return '';
  const text = escapeHtml(rawTextContent.value);
  return text
    .replace(/^### (.*$)/gim, '<h3 class="text-base font-bold mb-1.5 mt-3 text-gray-900 dark:text-white">$1</h3>')
    .replace(/^## (.*$)/gim, '<h2 class="text-lg font-bold border-b border-gray-200 dark:border-slate-800 pb-1 mb-2 mt-4 text-gray-900 dark:text-white">$1</h2>')
    .replace(/^# (.*$)/gim, '<h1 class="text-xl font-bold border-b border-gray-200 dark:border-slate-800 pb-1 mb-2 text-gray-900 dark:text-white">$1</h1>')
    .replace(/^\> (.*$)/gim, '<blockquote class="border-l-4 border-blue-500 pl-3 py-1 text-gray-600 dark:text-slate-400 italic my-2 bg-blue-50/20 dark:bg-blue-950/20 rounded-r">$1</blockquote>')
    .replace(/\*\*(.*?)\*\*/gim, '<strong class="font-bold text-gray-900 dark:text-white">$1</strong>')
    .replace(/\*(.*?)\*/gim, '<em class="italic">$1</em>')
    .replace(/`([^`]+)`/gim, '<code class="bg-gray-100 dark:bg-slate-800 px-1.5 py-0.5 rounded font-mono text-blue-600 dark:text-blue-400 text-xs">$1</code>')
    .replace(/\n$/gim, '<br />')
    .replace(/\n/gim, '<br />');
});

function closeAllDropdowns() {
  isSettingsOpen.value = false;
  isSyntaxMenuOpen.value = false;
}

function selectLanguage(lang: { name: string; mode: string }) {
  currentMode.value = lang.mode;
  if (editor.value) {
    editor.value.session.setMode(lang.mode);
  }
  isSyntaxMenuOpen.value = false;
}

function updateLanguageFromSelect() {
  if (editor.value) {
    editor.value.session.setMode(currentMode.value);
  }
}

function detectMode(filename: string): string {
  const customMap: Record<string, string> = {
    '.gitignore': 'ace/mode/sh',
    '.env': 'ace/mode/sh',
    '.dockerignore': 'ace/mode/sh',
    'Dockerfile': 'ace/mode/dockerfile',
    'Cargo.toml': 'ace/mode/toml',
    'Cargo.lock': 'ace/mode/toml',
  };

  if (customMap[filename]) return customMap[filename];

  const ext = filename.split('.').pop()?.toLowerCase();
  if (ext === 'rs') return 'ace/mode/rust';
  if (ext === 'toml') return 'ace/mode/toml';
  if (ext === 'ts') return 'ace/mode/typescript';
  if (ext === 'vue') return 'ace/mode/html';
  if (ext === 'md') return 'ace/mode/markdown';

  const mode = modelist.getModeForPath(filename).mode;
  return mode || 'ace/mode/text';
}

let markdownDebounceTimer: any = null;

function updateMarkdownPreviewDebounced() {
  if (!isMarkdownFile.value || !showMarkdownPreview.value) return;
  if (markdownDebounceTimer) clearTimeout(markdownDebounceTimer);
  markdownDebounceTimer = setTimeout(() => {
    if (editor.value) {
      rawTextContent.value = editor.value.getValue();
      charCount.value = rawTextContent.value.length;
    }
  }, 300);
}

watch(showMarkdownPreview, (show) => {
  if (show && editor.value) {
    rawTextContent.value = editor.value.getValue();
    charCount.value = rawTextContent.value.length;
  }
});

function initAce() {
  if (!editorEl.value || !uiStore.editorFile) return;

  editor.value = ace.edit(editorEl.value, {
    mode: detectMode(uiStore.editorFile.name),
    theme: editorTheme.value,
    fontSize: fontSize.value,
    fontFamily: fontFamily.value,
    tabSize: tabSize.value,
    wrap: wordWrap.value,
    showGutter: showGutter.value,
    showPrintMargin: false,
    useWorker: false,
    value: uiStore.editorContent,
  });

  currentMode.value = detectMode(uiStore.editorFile.name);
  lineCount.value = editor.value.session.getLength();
  charCount.value = uiStore.editorContent.length;
  if (isMarkdownFile.value && showMarkdownPreview.value) {
    rawTextContent.value = uiStore.editorContent;
  }

  editor.value.session.on('change', () => {
    isDirty.value = !editor.value?.session.getUndoManager().isClean();
    lineCount.value = editor.value?.session.getLength() || 1;
    if (isMarkdownFile.value && showMarkdownPreview.value) {
      updateMarkdownPreviewDebounced();
    }
  });

  editor.value.commands.addCommand({
    name: 'save',
    bindKey: { win: 'Ctrl-S', mac: 'Command-S' },
    exec: () => {
      handleSave();
    },
  });

  editor.value.focus();
}

function updateEditorTheme() {
  if (editor.value) {
    editor.value.setTheme(editorTheme.value);
    localStorage.setItem('fb:editor:theme', editorTheme.value);
  }
}

function updateEditorFont() {
  if (editor.value) {
    editor.value.setFontSize(fontSize.value);
    editor.value.setOption('fontFamily', fontFamily.value);
    localStorage.setItem('fb:editor:fontSize', fontSize.value.toString());
    localStorage.setItem('fb:editor:fontFamily', fontFamily.value);
  }
}

function updateEditorTabSize() {
  if (editor.value) {
    editor.value.session.setTabSize(tabSize.value);
    localStorage.setItem('fb:editor:tabSize', tabSize.value.toString());
  }
}

function updateEditorWrap() {
  if (editor.value) {
    editor.value.session.setUseWrapMode(wordWrap.value);
    localStorage.setItem('fb:editor:wordWrap', wordWrap.value ? 'true' : 'false');
  }
}

function updateEditorGutter() {
  if (editor.value) {
    editor.value.renderer.setShowGutter(showGutter.value);
    localStorage.setItem('fb:editor:showGutter', showGutter.value ? 'true' : 'false');
  }
}

watch(
  () => uiStore.isEditorOpen,
  (open) => {
    if (open && uiStore.editorFile) {
      isDirty.value = false;
      showMarkdownPreview.value = false;
      isSyntaxMenuOpen.value = false;
      isSettingsOpen.value = false;
      nextTick(() => {
        initAce();
      });
    } else {
      editor.value?.destroy();
      editor.value = null;
    }
  }
);

watch(
  () => themeStore.isDark,
  (dark) => {
    if (!localStorage.getItem('fb:editor:theme') && editor.value) {
      editorTheme.value = dark ? 'ace/theme/tomorrow_night' : 'ace/theme/chrome';
      editor.value.setTheme(editorTheme.value);
    }
  }
);

async function handleSave() {
  if (!uiStore.editorFile || !editor.value) return;
  saving.value = true;

  const currentText = editor.value.getValue();

  try {
    const headers: Record<string, string> = {};
    if (uiStore.editorEtag) {
      headers['If-Match'] = uiStore.editorEtag;
    }

    const resp = await apiClient.put(
      `/connections/${fileStore.currentConnectionId}/files/content`,
      {
        path: uiStore.editorFile.path,
        content: currentText,
      },
      { headers }
    );

    if (resp.headers['etag']) {
      uiStore.editorEtag = resp.headers['etag'];
    }

    editor.value.session.getUndoManager().markClean();
    isDirty.value = false;
    uiStore.showToast(`Saved ${uiStore.editorFile.name}`, 'success');

    // Auto reload workspace entries
    await workspaceStore.refreshAll();
  } catch (err: any) {
    if (err.response?.status === 409 || err.response?.status === 412) {
      const force = window.confirm(
        'Conflict Detected: This file was modified on disk since you opened it.\n\nDo you want to overwrite the remote changes? Click OK to overwrite, or Cancel to keep your local edits.'
      );
      if (force) {
        try {
          const forceResp = await apiClient.put(
            `/connections/${fileStore.currentConnectionId}/files/content`,
            {
              path: uiStore.editorFile.path,
              content: currentText,
            },
            {
              headers: {
                'X-Force-Overwrite': 'true',
              },
            }
          );
          if (forceResp.headers['etag']) {
            uiStore.editorEtag = forceResp.headers['etag'];
          }
          editor.value.session.getUndoManager().markClean();
          isDirty.value = false;
          uiStore.showToast(`Force saved ${uiStore.editorFile.name}`, 'warning');
          await workspaceStore.refreshAll();
          return;
        } catch (forceErr: any) {
          uiStore.showToast(forceErr.response?.data?.error?.message || 'Force save failed', 'error');
        }
      }
    } else {
      uiStore.showToast(err.response?.data?.error?.message || 'Failed to save file', 'error');
    }
  } finally {
    saving.value = false;
  }
}

function handleClose() {
  if (isDirty.value) {
    if (confirm('You have unsaved changes. Are you sure you want to discard them?')) {
      uiStore.isEditorOpen = false;
    }
  } else {
    uiStore.isEditorOpen = false;
  }
}

onBeforeUnmount(() => {
  editor.value?.destroy();
});
</script>

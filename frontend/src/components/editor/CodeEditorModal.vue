<template>
  <div
    v-if="uiStore.isEditorOpen && uiStore.editorFile"
    class="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex flex-col p-2 sm:p-4 select-none font-sans text-xs animate-in fade-in duration-150"
    @click="closeAllDropdowns"
  >
    <div
      class="bg-white dark:bg-[#0b0f19] border border-gray-200 dark:border-slate-800 rounded-3xl flex-1 flex flex-col shadow-2xl overflow-hidden"
      @click.stop
    >
      <!-- Editor Top Toolbar (Header) -->
      <div class="h-14 bg-gray-50 dark:bg-[#090d16] border-b border-gray-200 dark:border-slate-800 px-4 flex items-center justify-between text-xs shrink-0 gap-3">
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

      <!-- Quick Preferences Sub-Bar (When isSettingsOpen is true) -->
      <div
        v-if="isSettingsOpen"
        class="bg-gray-100 dark:bg-[#0f172a] border-b border-gray-200 dark:border-slate-800 px-4 py-2.5 flex flex-wrap items-center gap-4 text-xs select-none animate-in slide-in-from-top-2 duration-150 text-gray-800 dark:text-slate-200"
      >
        <!-- 1. Theme / Style -->
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

        <!-- 2. Font Family -->
        <div class="flex items-center space-x-1.5">
          <span class="text-gray-500 dark:text-slate-400 font-medium">Font:</span>
          <select
            v-model="fontFamily"
            @change="updateEditorFont"
            class="bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-xl px-2.5 py-1 text-gray-800 dark:text-slate-100 font-medium cursor-pointer shadow-xs focus:outline-none"
          >
            <option class="bg-white dark:bg-slate-900 text-gray-800 dark:text-slate-100" value="'JetBrains Mono', monospace">JetBrains Mono</option>
            <option class="bg-white dark:bg-slate-900 text-gray-800 dark:text-slate-100" value="'Fira Code', monospace">Fira Code</option>
            <option class="bg-white dark:bg-slate-900 text-gray-800 dark:text-slate-100" value="'SF Mono', 'Menlo', monospace">SF Mono / Menlo</option>
            <option class="bg-white dark:bg-slate-900 text-gray-800 dark:text-slate-100" value="'Cascadia Code', 'Consolas', monospace">Cascadia Code</option>
            <option class="bg-white dark:bg-slate-900 text-gray-800 dark:text-slate-100" value="'Ubuntu Mono', monospace">Ubuntu Mono</option>
            <option class="bg-white dark:bg-slate-900 text-gray-800 dark:text-slate-100" value="'Courier New', monospace">Courier New</option>
          </select>
        </div>

        <!-- 3. Font Size -->
        <div class="flex items-center space-x-1.5">
          <span class="text-gray-500 dark:text-slate-400 font-medium">Size:</span>
          <select
            v-model.number="fontSize"
            @change="updateEditorFont"
            class="bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-xl px-2.5 py-1 text-gray-800 dark:text-slate-100 font-medium cursor-pointer shadow-xs focus:outline-none"
          >
            <option class="bg-white dark:bg-slate-900 text-gray-800 dark:text-slate-100" :value="11">11 px</option>
            <option class="bg-white dark:bg-slate-900 text-gray-800 dark:text-slate-100" :value="12">12 px</option>
            <option class="bg-white dark:bg-slate-900 text-gray-800 dark:text-slate-100" :value="13">13 px</option>
            <option class="bg-white dark:bg-slate-900 text-gray-800 dark:text-slate-100" :value="14">14 px</option>
            <option class="bg-white dark:bg-slate-900 text-gray-800 dark:text-slate-100" :value="15">15 px</option>
            <option class="bg-white dark:bg-slate-900 text-gray-800 dark:text-slate-100" :value="16">16 px</option>
            <option class="bg-white dark:bg-slate-900 text-gray-800 dark:text-slate-100" :value="18">18 px</option>
            <option class="bg-white dark:bg-slate-900 text-gray-800 dark:text-slate-100" :value="20">20 px</option>
          </select>
        </div>

        <!-- 4. Tab Size -->
        <div class="flex items-center space-x-1.5">
          <span class="text-gray-500 dark:text-slate-400 font-medium">Tab:</span>
          <select
            v-model.number="tabSize"
            @change="updateEditorTabSize"
            class="bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-xl px-2.5 py-1 text-gray-800 dark:text-slate-100 font-medium cursor-pointer shadow-xs focus:outline-none"
          >
            <option class="bg-white dark:bg-slate-900 text-gray-800 dark:text-slate-100" :value="2">2 Spaces</option>
            <option class="bg-white dark:bg-slate-900 text-gray-800 dark:text-slate-100" :value="4">4 Spaces</option>
            <option class="bg-white dark:bg-slate-900 text-gray-800 dark:text-slate-100" :value="8">8 Spaces</option>
          </select>
        </div>

        <!-- 5. Word Wrap Toggle -->
        <label class="flex items-center space-x-1.5 cursor-pointer">
          <input
            type="checkbox"
            v-model="wordWrap"
            @change="updateEditorWrap"
            class="rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
          />
          <span class="text-gray-700 dark:text-slate-300 font-medium">Word Wrap</span>
        </label>

        <!-- 6. Line Numbers Toggle -->
        <label class="flex items-center space-x-1.5 cursor-pointer">
          <input
            type="checkbox"
            v-model="showGutter"
            @change="updateEditorGutter"
            class="rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
          />
          <span class="text-gray-700 dark:text-slate-300 font-medium">Line Numbers</span>
        </label>
      </div>

      <!-- Main Editor Body & Markdown Preview Split -->
      <div class="flex-1 flex overflow-hidden relative bg-white dark:bg-[#0b0f19]">
        <!-- Ace Code Editor Mount Point -->
        <div
          ref="editorEl"
          :class="[
            'h-full text-sm font-mono transition-all',
            showMarkdownPreview ? 'w-1/2 border-r border-gray-200 dark:border-slate-800' : 'w-full'
          ]"
        ></div>

        <!-- Markdown Live Render Preview -->
        <div
          v-if="showMarkdownPreview"
          class="w-1/2 h-full overflow-y-auto p-6 bg-white dark:bg-slate-900 text-gray-900 dark:text-slate-100 prose dark:prose-invert max-w-none text-xs"
          v-html="renderedMarkdown"
        ></div>
      </div>

      <!-- Editor Footer Status Bar -->
      <div class="h-8 bg-gray-50 dark:bg-[#090d16] border-t border-gray-200 dark:border-slate-800 px-4 flex items-center justify-between text-[11px] text-gray-500 dark:text-slate-500 select-none shrink-0 font-mono">
        <div class="flex items-center space-x-4">
          <span>Ace Editor v{{ aceVersion }}</span>
          <span>UTF-8</span>
          <span>Tab: {{ tabSize }} spaces</span>
          <span>{{ lineCount }} lines ({{ charCount }} chars)</span>
        </div>
        <div class="flex items-center space-x-3">
          <span v-if="isDirty" class="text-amber-500 font-semibold flex items-center space-x-1">
            <span>●</span>
            <span>Unsaved Changes</span>
          </span>
          <span v-else class="text-emerald-500 font-semibold flex items-center space-x-1">
            <span>●</span>
            <span>Saved</span>
          </span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onBeforeUnmount } from 'vue';
import ace, { version as aceVersion } from 'ace-builds';
import 'ace-builds/src-noconflict/ext-language_tools';
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

// Simple markdown parser for live preview
const renderedMarkdown = computed(() => {
  if (!rawTextContent.value) return '';
  const text = rawTextContent.value;
  return text
    .replace(/^# (.*$)/gim, '<h1 class="text-xl font-bold border-b pb-1 mb-2">$1</h1>')
    .replace(/^## (.*$)/gim, '<h2 class="text-lg font-bold border-b pb-1 mb-2 mt-4">$1</h2>')
    .replace(/^### (.*$)/gim, '<h3 class="text-base font-bold mb-1.5 mt-3">$1</h3>')
    .replace(/\*\*(.*)\*\*/gim, '<strong>$1</strong>')
    .replace(/\*(.*)\*/gim, '<em>$1</em>')
    .replace(/`([^`]+)`/gim, '<code class="bg-gray-100 dark:bg-slate-800 px-1.5 py-0.5 rounded font-mono text-[11px]">$1</code>')
    .replace(/\n\n/gim, '<br/><br/>')
    .replace(/\n/gim, '<br/>');
});

function closeAllDropdowns() {
  isSyntaxMenuOpen.value = false;
}

function selectLanguage(lang: { name: string; mode: string }) {
  currentMode.value = lang.mode;
  if (editor.value) {
    editor.value.session.setMode(lang.mode);
  }
  isSyntaxMenuOpen.value = false;
}

function initAce() {
  if (!editorEl.value || !uiStore.editorFile) return;

  // Auto-detect mode from filename
  const autoMode = modelist.getModeForPath(uiStore.editorFile.name).mode;
  currentMode.value = autoMode;

  // Theme adaptation based on current dark setting if not customized
  if (!localStorage.getItem('fb:editor:theme')) {
    editorTheme.value = themeStore.isDark ? 'ace/theme/tomorrow_night' : 'ace/theme/chrome';
  }

  rawTextContent.value = uiStore.editorContent;

  editor.value = ace.edit(editorEl.value, {
    value: uiStore.editorContent,
    theme: editorTheme.value,
    mode: currentMode.value,
    showPrintMargin: false,
    fontSize: fontSize.value,
    fontFamily: fontFamily.value,
    wrap: wordWrap.value,
    showGutter: showGutter.value,
    enableBasicAutocompletion: true,
    enableLiveAutocompletion: true,
    enableSnippets: true,
    tabSize: tabSize.value,
    useSoftTabs: true,
  });

  lineCount.value = editor.value.session.getLength();
  charCount.value = uiStore.editorContent.length;

  editor.value.on('change', () => {
    isDirty.value = !editor.value?.session.getUndoManager().isClean();
    lineCount.value = editor.value?.session.getLength() || 1;
    rawTextContent.value = editor.value?.getValue() || '';
    charCount.value = rawTextContent.value.length;
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

  try {
    const headers: Record<string, string> = {};
    if (uiStore.editorEtag) {
      headers['If-Match'] = uiStore.editorEtag;
    }

    const currentText = editor.value.getValue();

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
    if (err.response?.status === 409) {
      uiStore.showToast('Conflict: File was modified externally. Please reload.', 'error');
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

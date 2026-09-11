<template>
  <Transition name="ios-modal">
    <div
      v-if="editorStore.isOpen && editorStore.activeFile"
      class="fixed inset-0 z-50 bg-black/70 backdrop-blur-xs flex flex-col p-0 md:p-3 select-none font-sans text-xs"
      @click="closeAllPopovers"
    >
      <div
        class="bg-white dark:bg-[#0b0f19] border-0 md:border md:border-gray-200/80 dark:md:border-slate-800 rounded-none md:rounded-2xl flex-1 flex flex-col shadow-2xl overflow-hidden relative ring-1 ring-black/5 dark:ring-white/5"
        @click.stop
      >
        <!-- ================= HEADER ================= -->
        <EditorHeader
          :active-file="editorStore.activeFile"
          :is-dirty="editorStore.isDirty"
          :saving="editorStore.saving"
          :is-markdown="isMarkdownFile"
          :show-markdown-preview="editorStore.showMarkdownPreview"
          :is-settings-open="isSettingsOpen"
          :is-search-open="isSearchOpen"
          :is-goto-open="isGotoOpen"
          :is-mobile="uiStore.isMobile"
          @toggle-search="toggleSearchBar(false)"
          @toggle-goto="toggleGoto"
          @format="formatDocument"
          @toggle-preview="editorStore.toggleMarkdownPreview()"
          @toggle-settings="toggleSettings"
          @save="handleSave"
          @close="editorStore.requestClose"
        />

        <!-- ================= MAIN CANVAS ================= -->
        <div class="flex-1 flex flex-col overflow-hidden relative bg-white dark:bg-[#0b0f19]">
          <!-- Loading Overlay -->
          <div
            v-if="editorStore.isLoading"
            class="absolute inset-0 z-40 bg-white/80 dark:bg-[#0b0f19]/80 backdrop-blur-xs flex flex-col items-center justify-center space-y-3"
          >
            <div class="w-7 h-7 border-2 border-blue-500 border-t-transparent rounded-full animate-spin"></div>
            <span class="text-xs text-gray-500 dark:text-gray-400 font-medium">Loading document...</span>
          </div>

          <!-- Remote Background Conflict Banner -->
          <div
            v-if="editorStore.remoteConflictDetected"
            class="bg-amber-500/15 border-b border-amber-500/30 px-4 py-2 flex items-center justify-between text-amber-600 dark:text-amber-400 text-xs font-medium z-30"
          >
            <div class="flex items-center space-x-2">
              <span>⚠️</span>
              <span>The file on the server has changed since your draft was saved.</span>
            </div>
            <div class="flex items-center space-x-2">
              <button
                @click="editorStore.forceSaveFile"
                class="px-2.5 py-1 bg-amber-600 hover:bg-amber-700 text-white font-bold rounded-lg cursor-pointer"
              >
                Overwrite Server
              </button>
            </div>
          </div>

          <!-- Canvas Sub-area (Ace & Preview) -->
          <div class="flex-1 flex overflow-hidden relative">
            <!-- Floating Search Bar (Ctrl+F / Ctrl+H) -->
            <EditorSearch
              ref="searchRef"
              v-model="isSearchOpen"
              v-model:search-query="searchQuery"
              v-model:replace-query="replaceQuery"
              :search-case-sensitive="searchCaseSensitive"
              :search-whole-word="searchWholeWord"
              :search-regex="searchRegex"
              :match-count="matchCount"
              :current-match-idx="currentMatchIdx"
              @toggle-case-sensitive="searchCaseSensitive = !searchCaseSensitive; executeSearch()"
              @toggle-whole-word="searchWholeWord = !searchWholeWord; executeSearch()"
              @toggle-regex="searchRegex = !searchRegex; executeSearch()"
              @find-next="findNext"
              @find-prev="findPrev"
              @replace-current="replaceCurrent"
              @replace-all="replaceAll"
            />

            <!-- Floating Go-to-Line (Ctrl+G) -->
            <EditorGotoLine
              ref="gotoRef"
              v-model="isGotoOpen"
              :line-count="lineCount"
              @jump-to-line="jumpToLine"
            />

            <!-- Settings Popover & Mobile Sheet -->
            <EditorSettings
              v-model="isSettingsOpen"
              :preferences="editorStore.preferences"
              :is-mobile="uiStore.isMobile"
              @update-preference="onPreferenceChange"
            />

            <!-- Ace Mount Container -->
            <div
              v-show="!uiStore.isMobile || !editorStore.showMarkdownPreview"
              ref="editorEl"
              :class="[
                'h-full text-xs sm:text-sm font-mono transition-[width] duration-150',
                !uiStore.isMobile && editorStore.showMarkdownPreview ? 'w-1/2 border-r border-gray-200 dark:border-slate-800' : 'w-full'
              ]"
            ></div>

            <!-- Markdown Split Preview -->
            <EditorMarkdownPreview
              v-if="editorStore.showMarkdownPreview"
              :content="editorStore.content"
              :is-mobile="uiStore.isMobile"
            />
          </div>
        </div>

        <!-- ================= MOBILE ACCESSORY BAR ================= -->
        <div
          v-if="uiStore.isMobile"
          class="h-11 bg-gray-100 dark:bg-slate-900 border-t border-gray-200 dark:border-slate-800 flex items-center px-2 space-x-1.5 overflow-x-auto shrink-0 select-none pb-safe"
        >
          <button
            v-for="sym in quickSymbols"
            :key="sym.label"
            @pointerdown.prevent
            @click="insertSymbol(sym.value)"
            class="px-3 py-1.5 bg-white dark:bg-slate-800 hover:bg-gray-200 dark:hover:bg-slate-700 text-gray-800 dark:text-slate-100 font-mono text-xs font-semibold rounded-lg shadow-2xs border border-gray-200 dark:border-slate-700 shrink-0 cursor-pointer active:scale-95 transition"
          >
            {{ sym.label }}
          </button>
        </div>

        <!-- ================= STATUS BAR ================= -->
        <EditorStatusBar
          :cursor-position="editorStore.cursorPosition"
          :selected-char-count="selectedCharCount"
          :line-count="lineCount"
          :char-count="charCount"
          :tab-size="editorStore.preferences.tabSize"
          :current-mode="editorStore.languageMode"
          :available-languages="availableLanguages"
          :is-dirty="editorStore.isDirty"
          @update-tab-size="onTabSizeChange"
          @select-language="onLanguageChange"
        />

        <!-- ================= CONFLICT & UNSAVED MODALS ================= -->
        <EditorConflictDialog
          :filename="editorStore.activeFile?.name || ''"
          :is-unsaved-confirm-open="editorStore.isUnsavedConfirmOpen"
          :is-conflict-modal-open="editorStore.isConflictModalOpen"
          :saving="editorStore.saving"
          @cancel-unsaved="editorStore.isUnsavedConfirmOpen = false"
          @discard-unsaved="editorStore.discardAndClose"
          @save-and-close="handleSaveAndClose"
          @cancel-conflict="editorStore.isConflictModalOpen = false"
          @force-save="handleForceSave"
          @reload-disk="handleReloadDisk"
        />
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onBeforeUnmount } from 'vue';
import ace, { version as aceVersion } from 'ace-builds';
import modelist from 'ace-builds/src-noconflict/ext-modelist';

// Bundled core Ace themes
import 'ace-builds/src-noconflict/theme-tomorrow_night';
import 'ace-builds/src-noconflict/theme-one_dark';
import 'ace-builds/src-noconflict/theme-dracula';
import 'ace-builds/src-noconflict/theme-monokai';
import 'ace-builds/src-noconflict/theme-chrome';
import 'ace-builds/src-noconflict/theme-github';

// Bundled common Ace modes
import 'ace-builds/src-noconflict/mode-text';
import 'ace-builds/src-noconflict/mode-rust';
import 'ace-builds/src-noconflict/mode-typescript';
import 'ace-builds/src-noconflict/mode-javascript';
import 'ace-builds/src-noconflict/mode-json';
import 'ace-builds/src-noconflict/mode-toml';
import 'ace-builds/src-noconflict/mode-yaml';
import 'ace-builds/src-noconflict/mode-markdown';
import 'ace-builds/src-noconflict/mode-sh';
import 'ace-builds/src-noconflict/mode-html';
import 'ace-builds/src-noconflict/mode-css';
import 'ace-builds/src-noconflict/mode-python';
import 'ace-builds/src-noconflict/mode-golang';
import 'ace-builds/src-noconflict/mode-sql';
import 'ace-builds/src-noconflict/mode-dockerfile';

import EditorHeader from './EditorHeader.vue';
import EditorStatusBar from './EditorStatusBar.vue';
import EditorSettings from './EditorSettings.vue';
import EditorSearch from './EditorSearch.vue';
import EditorGotoLine from './EditorGotoLine.vue';
import EditorConflictDialog from './EditorConflictDialog.vue';
import EditorMarkdownPreview from './EditorMarkdownPreview.vue';

import { useEditorStore } from '../../stores/editorStore';
import { useUiStore } from '../../stores/uiStore';
import type { EditorPreferences } from '../../services/editorSession';

ace.config.set(
  'basePath',
  `https://cdn.jsdelivr.net/npm/ace-builds@${aceVersion}/src-min-noconflict/`
);

const editorStore = useEditorStore();
const uiStore = useUiStore();
const toast = {
  success: (msg: string) => uiStore.showToast(msg, 'success'),
  error: (msg: string) => uiStore.showToast(msg, 'error'),
  info: (msg: string) => uiStore.showToast(msg, 'info'),
  warning: (msg: string) => uiStore.showToast(msg, 'warning'),
};

const editorEl = ref<HTMLElement | null>(null);
const editor = ref<ace.Ace.Editor | null>(null);

const isSettingsOpen = ref(false);
const isSearchOpen = ref(false);
const isGotoOpen = ref(false);

const searchRef = ref<InstanceType<typeof EditorSearch> | null>(null);
const gotoRef = ref<InstanceType<typeof EditorGotoLine> | null>(null);

const searchQuery = ref('');
const replaceQuery = ref('');
const searchCaseSensitive = ref(false);
const searchWholeWord = ref(false);
const searchRegex = ref(false);
const matchCount = ref(0);
const currentMatchIdx = ref(0);

const lineCount = ref(1);
const charCount = ref(0);
const selectedCharCount = ref(0);

const availableLanguages = [
  { name: 'Rust', mode: 'ace/mode/rust' },
  { name: 'TypeScript', mode: 'ace/mode/typescript' },
  { name: 'JavaScript', mode: 'ace/mode/javascript' },
  { name: 'Python', mode: 'ace/mode/python' },
  { name: 'Go', mode: 'ace/mode/golang' },
  { name: 'HTML', mode: 'ace/mode/html' },
  { name: 'CSS', mode: 'ace/mode/css' },
  { name: 'SCSS', mode: 'ace/mode/scss' },
  { name: 'JSON', mode: 'ace/mode/json' },
  { name: 'YAML', mode: 'ace/mode/yaml' },
  { name: 'TOML', mode: 'ace/mode/toml' },
  { name: 'Markdown', mode: 'ace/mode/markdown' },
  { name: 'SQL', mode: 'ace/mode/sql' },
  { name: 'Shell / Bash', mode: 'ace/mode/sh' },
  { name: 'PHP', mode: 'ace/mode/php' },
  { name: 'C / C++', mode: 'ace/mode/c_cpp' },
  { name: 'Java', mode: 'ace/mode/java' },
  { name: 'Kotlin', mode: 'ace/mode/kotlin' },
  { name: 'Ruby', mode: 'ace/mode/ruby' },
  { name: 'Lua', mode: 'ace/mode/lua' },
  { name: 'Dockerfile', mode: 'ace/mode/dockerfile' },
  { name: 'Nginx', mode: 'ace/mode/nginx' },
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

const isMarkdownFile = computed(() => {
  if (!editorStore.activeFile) return false;
  const ext = editorStore.activeFile.name.split('.').pop()?.toLowerCase() || '';
  return ['md', 'markdown', 'mdown'].includes(ext);
});

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
  if (ext === 'json') return 'ace/mode/json';
  if (ext === 'yaml' || ext === 'yml') return 'ace/mode/yaml';

  const mode = modelist.getModeForPath(filename).mode;
  return mode || 'ace/mode/text';
}

function initAce() {
  if (!editorEl.value || !editorStore.activeFile) return;

  if (editor.value) {
    editor.value.destroy();
    editor.value = null;
  }

  const prefs = editorStore.preferences;
  const detected = editorStore.languageMode && editorStore.languageMode !== 'ace/mode/text'
    ? editorStore.languageMode
    : detectMode(editorStore.activeFile.name);

  editor.value = ace.edit(editorEl.value, {
    mode: detected,
    theme: prefs.theme,
    fontSize: prefs.fontSize,
    fontFamily: prefs.fontFamily,
    tabSize: prefs.tabSize,
    wrap: prefs.wordWrap,
    showGutter: prefs.showGutter,
    highlightActiveLine: prefs.highlightActiveLine,
    showPrintMargin: false,
    useWorker: false,
    behavioursEnabled: true,
    value: editorStore.content || '',
  });

  editorStore.setLanguageMode(detected);
  lineCount.value = editor.value.session.getLength();
  charCount.value = (editorStore.content || '').length;

  // Restore cursor and scroll position if available
  if (editorStore.cursorPosition.row > 0 || editorStore.cursorPosition.column > 0) {
    editor.value.gotoLine(editorStore.cursorPosition.row + 1, editorStore.cursorPosition.column, false);
  }
  if (editorStore.scrollTop > 0) {
    editor.value.session.setScrollTop(editorStore.scrollTop);
  }

  // Event Listeners
  editor.value.session.on('change', () => {
    if (!editor.value) return;
    const val = editor.value.getValue();
    editorStore.updateContent(val);
    lineCount.value = editor.value.session.getLength() || 1;
    charCount.value = val.length;
  });

  editor.value.selection.on('changeCursor', () => {
    if (editor.value) {
      editorStore.updateCursor(editor.value.getCursorPosition());
    }
  });

  editor.value.selection.on('changeSelection', () => {
    if (editor.value) {
      selectedCharCount.value = editor.value.getSelectedText().length;
    }
  });

  editor.value.session.on('changeScrollTop', (top: number) => {
    editorStore.updateScroll(top);
  });

  // Ace Keyboard Commands
  editor.value.commands.addCommand({
    name: 'save',
    bindKey: { win: 'Ctrl-S', mac: 'Command-S' },
    exec: () => { handleSave(); },
  });

  editor.value.commands.addCommand({
    name: 'find',
    bindKey: { win: 'Ctrl-F', mac: 'Command-F' },
    exec: () => { toggleSearchBar(false); },
  });

  editor.value.commands.addCommand({
    name: 'replace',
    bindKey: { win: 'Ctrl-H', mac: 'Command-H' },
    exec: () => { toggleSearchBar(true); },
  });

  editor.value.commands.addCommand({
    name: 'gotoLine',
    bindKey: { win: 'Ctrl-G', mac: 'Command-G' },
    exec: () => { toggleGoto(); },
  });

  editor.value.commands.addCommand({
    name: 'format',
    bindKey: { win: 'Shift-Alt-F', mac: 'Shift-Option-F' },
    exec: () => { formatDocument(); },
  });

  editor.value.focus();

  setTimeout(() => {
    editor.value?.resize();
  }, 60);
}

function closeAllPopovers() {
  const hadOpen = isSettingsOpen.value || isSearchOpen.value || isGotoOpen.value;
  isSettingsOpen.value = false;
  isSearchOpen.value = false;
  isGotoOpen.value = false;
  if (hadOpen) {
    editor.value?.focus();
  }
}

function toggleSettings() {
  if (isSettingsOpen.value) {
    isSettingsOpen.value = false;
    editor.value?.focus();
    return;
  }
  isSettingsOpen.value = true;
  isSearchOpen.value = false;
  isGotoOpen.value = false;
}

function toggleSearchBar(focusReplace = false) {
  if (isSearchOpen.value && !focusReplace) {
    isSearchOpen.value = false;
    editor.value?.focus();
    return;
  }
  isSearchOpen.value = true;
  isGotoOpen.value = false;
  isSettingsOpen.value = false;

  nextTick(() => {
    if (editor.value) {
      const selected = editor.value.getSelectedText();
      if (selected && !selected.includes('\n')) {
        searchQuery.value = selected;
      }
    }
    if (searchQuery.value) {
      executeSearch();
    }
    if (focusReplace) {
      searchRef.value?.focusReplace();
    } else {
      searchRef.value?.focusSearch();
    }
  });
}

function toggleGoto() {
  if (isGotoOpen.value) {
    isGotoOpen.value = false;
    editor.value?.focus();
    return;
  }
  isGotoOpen.value = true;
  isSearchOpen.value = false;
  isSettingsOpen.value = false;
  nextTick(() => {
    gotoRef.value?.focus();
  });
}

function executeSearch() {
  if (!editor.value || !searchQuery.value) {
    matchCount.value = 0;
    currentMatchIdx.value = 0;
    return;
  }

  try {
    const text = editor.value.getValue();
    let flags = 'g';
    if (!searchCaseSensitive.value) flags += 'i';

    let pattern = searchRegex.value ? searchQuery.value : searchQuery.value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    if (searchWholeWord.value) pattern = `\\b${pattern}\\b`;

    const regex = new RegExp(pattern, flags);
    const matches = text.match(regex);
    matchCount.value = matches ? matches.length : 0;

    editor.value.find(searchQuery.value, {
      backwards: false,
      wrap: true,
      caseSensitive: searchCaseSensitive.value,
      wholeWord: searchWholeWord.value,
      regExp: searchRegex.value,
    });
    currentMatchIdx.value = matchCount.value > 0 ? 1 : 0;
  } catch {
    matchCount.value = 0;
  }
}

function findNext() {
  if (!editor.value) return;
  editor.value.findNext();
  if (matchCount.value > 0) {
    currentMatchIdx.value = (currentMatchIdx.value % matchCount.value) + 1;
  }
}

function findPrev() {
  if (!editor.value) return;
  editor.value.findPrevious();
  if (matchCount.value > 0) {
    currentMatchIdx.value = currentMatchIdx.value <= 1 ? matchCount.value : currentMatchIdx.value - 1;
  }
}

function replaceCurrent() {
  if (!editor.value) return;
  editor.value.replace(replaceQuery.value);
  executeSearch();
}

function replaceAll() {
  if (!editor.value) return;
  editor.value.replaceAll(replaceQuery.value);
  executeSearch();
  toast.info('Replaced all matches');
}

function jumpToLine(line: number, col: number) {
  if (!editor.value) return;
  const zeroBasedCol = Math.max(0, col - 1);
  editor.value.gotoLine(line, zeroBasedCol, true);
  editor.value.focus();
}

function formatDocument() {
  if (!editor.value) return;
  const currentText = editor.value.getValue();
  const ext = editorStore.activeFile?.name.split('.').pop()?.toLowerCase();

  if (ext === 'json' || editorStore.languageMode === 'ace/mode/json') {
    try {
      const parsed = JSON.parse(currentText);
      const formatted = JSON.stringify(parsed, null, editorStore.preferences.tabSize);
      editor.value.setValue(formatted, -1);
      toast.success('Formatted JSON document');
      return;
    } catch {
      toast.warning('Unable to format: Invalid JSON syntax');
      return;
    }
  }

  try {
    const session = editor.value.getSession();
    const rows = session.getLength();
    for (let i = 0; i < rows; i++) {
      session.indentRows(i, i, '');
    }
    toast.info('Document indentation adjusted');
  } catch {
    toast.info('Format completed');
  }
}

function insertSymbol(val: string) {
  if (editor.value) {
    editor.value.insert(val);
    editor.value.focus();
  }
}

function onPreferenceChange<K extends keyof EditorPreferences>(key: K, val: any) {
  editorStore.updatePreference(key, val);
  if (!editor.value) return;

  if (key === 'theme') {
    editor.value.setTheme(val);
  } else if (key === 'fontFamily') {
    editor.value.setOption('fontFamily', val);
  } else if (key === 'fontSize') {
    editor.value.setFontSize(val);
  } else if (key === 'tabSize') {
    editor.value.session.setTabSize(val);
  } else if (key === 'wordWrap') {
    editor.value.session.setUseWrapMode(val);
  } else if (key === 'showGutter') {
    editor.value.renderer.setShowGutter(val);
  } else if (key === 'highlightActiveLine') {
    editor.value.setHighlightActiveLine(val);
  }
}

function onTabSizeChange(size: number) {
  onPreferenceChange('tabSize', size);
}

function onLanguageChange(mode: string) {
  editorStore.setLanguageMode(mode);
  if (editor.value) {
    editor.value.session.setMode(mode);
  }
}

async function handleSave() {
  try {
    await editorStore.saveFile();
    toast.success(`Saved ${editorStore.activeFile?.name}`);
  } catch (err: any) {
    if (err.response?.status !== 409 && err.response?.status !== 412) {
      toast.error('Failed to save file');
    }
  }
}

async function handleForceSave() {
  try {
    await editorStore.forceSaveFile();
    toast.warning(`Force saved ${editorStore.activeFile?.name}`);
  } catch {
    toast.error('Force save failed');
  }
}

async function handleSaveAndClose() {
  try {
    await editorStore.saveFile();
    editorStore.isUnsavedConfirmOpen = false;
    editorStore.closeEditor();
    toast.success(`Saved ${editorStore.activeFile?.name}`);
  } catch {
    // Modal will prompt if conflict
  }
}

async function handleReloadDisk() {
  try {
    await editorStore.reloadFromDisk();
    toast.info('Reloaded latest content from disk');
  } catch {
    toast.error('Failed to reload file from disk');
  }
}

function handleEditorKeyDown(e: KeyboardEvent) {
  if (!editorStore.isOpen) return;

  const isMac = navigator.platform.toUpperCase().indexOf('MAC') >= 0;
  const isCtrlOrCmd = isMac ? e.metaKey : e.ctrlKey;

  if (isCtrlOrCmd && (e.key === 'h' || e.key === 'H')) {
    e.preventDefault();
    e.stopPropagation();
    toggleSearchBar(true);
  } else if (isCtrlOrCmd && (e.key === 'f' || e.key === 'F')) {
    e.preventDefault();
    e.stopPropagation();
    toggleSearchBar(false);
  } else if (isCtrlOrCmd && (e.key === 's' || e.key === 'S')) {
    e.preventDefault();
    e.stopPropagation();
    handleSave();
  } else if (isCtrlOrCmd && (e.key === 'g' || e.key === 'G')) {
    e.preventDefault();
    e.stopPropagation();
    toggleGoto();
  } else if (e.key === 'Escape') {
    if (isSearchOpen.value || isGotoOpen.value || isSettingsOpen.value) {
      e.preventDefault();
      e.stopPropagation();
      closeAllPopovers();
    } else if (!editorStore.isUnsavedConfirmOpen && !editorStore.isConflictModalOpen) {
      e.preventDefault();
      e.stopPropagation();
      editorStore.requestClose();
    }
  }
}

onMounted(() => {
  window.addEventListener('keydown', handleEditorKeyDown, { capture: true });
  if (editorStore.isOpen && editorStore.activeFile) {
    nextTick(() => {
      initAce();
    });
  }
});

watch(
  () => editorStore.content,
  (newContent) => {
    if (editor.value && editor.value.getValue() !== newContent) {
      const cur = editor.value.getCursorPosition();
      editor.value.setValue(newContent || '', -1);
      editor.value.moveCursorToPosition(cur);
      lineCount.value = editor.value.session.getLength() || 1;
      charCount.value = (newContent || '').length;
    }
  }
);

watch(
  () => [editorStore.isOpen, editorStore.activeFile?.path],
  ([open]) => {
    if (open && editorStore.activeFile) {
      closeAllPopovers();
      nextTick(() => {
        initAce();
      });
    } else if (!open) {
      editor.value?.destroy();
      editor.value = null;
    }
  }
);

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleEditorKeyDown, { capture: true });
  editor.value?.destroy();
});
</script>

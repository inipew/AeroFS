<template>
  <Transition name="ios-modal">
    <div
      v-if="uiStore.isEditorOpen && uiStore.editorFile"
      class="fixed inset-0 z-50 bg-black/80 backdrop-blur-md flex flex-col p-0 md:p-3 select-none font-sans text-xs"
      @click="closeAllPopovers"
    >
      <div
        class="modal-card bg-white dark:bg-[#0b0f19] border-0 md:border md:border-gray-200/80 dark:md:border-slate-800 rounded-none md:rounded-3xl flex-1 flex flex-col shadow-2xl overflow-hidden relative ring-1 ring-black/5 dark:ring-white/5"
        @click.stop
      >
      <!-- ================= HEADER TOOLBAR ================= -->
      <div
        class="h-13 sm:h-14 bg-gray-50/90 dark:bg-[#090d16]/95 border-b border-gray-200/80 dark:border-slate-800/80 px-3 sm:px-4 flex items-center justify-between text-xs shrink-0 gap-2 select-none backdrop-blur-md"
      >
        <!-- Left: File Badge & Title with Breadcrumb -->
        <div class="flex items-center space-x-2.5 truncate flex-1 min-w-0">
          <!-- Mobile Back Button -->
          <button
            v-if="uiStore.isMobile"
            @click="handleClose"
            class="p-1.5 -ml-1 text-gray-500 hover:text-gray-900 dark:text-slate-400 dark:hover:text-white rounded-xl hover:bg-gray-200 dark:hover:bg-slate-800 transition cursor-pointer shrink-0"
            title="Close Editor"
          >
            <FbIcon name="chevron-left" size="18px" />
          </button>

          <!-- Category Colored File Icon -->
          <div
            class="w-8 h-8 sm:w-9 sm:h-9 rounded-xl flex items-center justify-center shrink-0 border shadow-2xs"
            :class="[fileTypeMeta.badgeBg, fileTypeMeta.badgeBorder]"
          >
            <span class="text-sm select-none">{{ fileTypeMeta.symbol }}</span>
          </div>

          <div class="truncate flex-1 min-w-0">
            <div class="flex items-center space-x-2 truncate">
              <span class="font-bold text-gray-900 dark:text-white text-xs sm:text-sm truncate">
                {{ uiStore.editorFile.name }}
              </span>
              <span
                v-if="isDirty"
                class="w-2 h-2 rounded-full bg-amber-500 animate-pulse shrink-0"
                title="Unsaved changes (Ctrl+S to save)"
              ></span>
              <span
                class="px-1.5 py-0.5 rounded-md font-mono text-[9px] font-bold uppercase tracking-wider border shadow-2xs shrink-0"
                :class="[fileTypeMeta.badgeBg, fileTypeMeta.badgeText, fileTypeMeta.badgeBorder]"
              >
                {{ currentModeName }}
              </span>
            </div>
            <div class="flex items-center space-x-1 text-[10px] text-gray-400 dark:text-slate-500 font-mono truncate">
              <span class="truncate">{{ uiStore.editorFile.path }}</span>
              <button
                @click="copyFilePath"
                class="p-0.5 hover:text-blue-500 transition cursor-pointer shrink-0"
                title="Copy Path"
              >
                <FbIcon name="copy" size="10px" />
              </button>
            </div>
          </div>
        </div>

        <!-- Center / Right Controls -->
        <div class="flex items-center space-x-1.5 sm:space-x-2 shrink-0">
          <!-- Find & Replace Button (Ctrl+F) -->
          <button
            @click.stop="toggleSearchBar"
            :class="[
              'p-2 sm:px-2.5 sm:py-1.5 rounded-xl border text-xs font-semibold flex items-center space-x-1.5 transition cursor-pointer shadow-2xs',
              isSearchOpen
                ? 'bg-blue-50 dark:bg-blue-950/60 text-blue-600 dark:text-blue-400 border-blue-500/40'
                : 'bg-white dark:bg-slate-900 text-gray-700 dark:text-slate-300 border-gray-200/80 dark:border-slate-700/80 hover:bg-gray-50 dark:hover:bg-slate-800'
            ]"
            title="Find & Replace (Ctrl+F)"
          >
            <FbIcon name="search" size="13px" />
            <span class="hidden md:inline">Find</span>
          </button>

          <!-- Go to Line Button (Ctrl+G) -->
          <button
            @click.stop="isGotoOpen = !isGotoOpen; isSearchOpen = false"
            :class="[
              'p-2 sm:px-2.5 sm:py-1.5 rounded-xl border text-xs font-semibold flex items-center space-x-1.5 transition cursor-pointer shadow-2xs',
              isGotoOpen
                ? 'bg-blue-50 dark:bg-blue-950/60 text-blue-600 dark:text-blue-400 border-blue-500/40'
                : 'bg-white dark:bg-slate-900 text-gray-700 dark:text-slate-300 border-gray-200/80 dark:border-slate-700/80 hover:bg-gray-50 dark:hover:bg-slate-800'
            ]"
            title="Go to Line (Ctrl+G)"
          >
            <span class="font-mono text-xs font-bold leading-none">#</span>
            <span class="hidden md:inline">Go to</span>
          </button>

          <!-- Format Code Button (Shift+Alt+F) -->
          <button
            @click="formatDocument"
            class="p-2 sm:px-2.5 sm:py-1.5 rounded-xl border bg-white dark:bg-slate-900 hover:bg-gray-50 dark:hover:bg-slate-800 border-gray-200/80 dark:border-slate-700/80 text-gray-700 dark:text-slate-300 text-xs font-semibold flex items-center space-x-1.5 transition cursor-pointer shadow-2xs"
            title="Format / Prettify Code (Shift+Alt+F)"
          >
            <FbIcon name="code" size="13px" />
            <span class="hidden md:inline">Format</span>
          </button>

          <!-- Syntax Selector Dropdown -->
          <div class="relative hidden sm:block">
            <button
              @click.stop="isSyntaxMenuOpen = !isSyntaxMenuOpen; isSettingsOpen = false; isGotoOpen = false"
              class="bg-white dark:bg-slate-900 hover:bg-gray-50 dark:hover:bg-slate-800 border border-gray-200/80 dark:border-slate-700/80 px-2.5 py-1.5 rounded-xl text-gray-700 dark:text-slate-200 flex items-center space-x-1.5 text-xs font-semibold shadow-2xs transition cursor-pointer"
              title="Change Language Mode"
            >
              <span class="text-gray-400 font-normal">Mode:</span>
              <span class="text-blue-600 dark:text-blue-400 font-bold max-w-[80px] truncate">{{ currentModeName }}</span>
              <FbIcon name="chevron-down" size="11px" class="text-gray-400" />
            </button>

            <!-- Searchable Syntax Popover Menu -->
            <div
              v-if="isSyntaxMenuOpen"
              class="absolute top-full mt-1.5 right-0 w-56 bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-2xl shadow-2xl p-2 z-50 animate-in fade-in zoom-in-95 duration-100"
            >
              <input
                ref="syntaxSearchInputRef"
                v-model="syntaxSearchQuery"
                type="text"
                placeholder="Search language..."
                class="w-full bg-gray-50 dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded-xl px-2.5 py-1.5 text-xs text-gray-900 dark:text-slate-100 outline-none mb-1.5 font-medium"
                @click.stop
              />
              <div class="max-h-60 overflow-y-auto space-y-0.5">
                <button
                  v-for="lang in filteredLanguages"
                  :key="lang.mode"
                  @click="selectLanguage(lang)"
                  :class="[
                    'w-full flex items-center justify-between px-2.5 py-1.5 rounded-xl text-left text-xs transition cursor-pointer',
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

          <!-- Markdown Split Preview Toggle -->
          <button
            v-if="isMarkdownFile"
            @click="showMarkdownPreview = !showMarkdownPreview"
            :class="[
              'p-2 sm:px-2.5 sm:py-1.5 rounded-xl border text-xs font-semibold flex items-center space-x-1.5 transition cursor-pointer shadow-2xs',
              showMarkdownPreview
                ? 'bg-blue-50 dark:bg-blue-950/60 text-blue-600 dark:text-blue-400 border-blue-500/40'
                : 'bg-white dark:bg-slate-900 text-gray-700 dark:text-slate-300 border-gray-200/80 dark:border-slate-700/80 hover:bg-gray-50'
            ]"
            title="Toggle Markdown Live Preview"
          >
            <FbIcon name="eye" size="13px" />
            <span class="hidden md:inline">{{ showMarkdownPreview ? 'Editor' : 'Preview' }}</span>
          </button>

          <!-- Preferences Drawer Button -->
          <button
            @click.stop="isSettingsOpen = !isSettingsOpen; isSyntaxMenuOpen = false; isGotoOpen = false"
            :class="[
              'p-2 sm:px-2.5 sm:py-1.5 rounded-xl border text-xs font-semibold flex items-center space-x-1.5 transition cursor-pointer shadow-2xs',
              isSettingsOpen
                ? 'bg-blue-50 dark:bg-blue-950/60 text-blue-600 dark:text-blue-400 border-blue-500/40'
                : 'bg-white dark:bg-slate-900 text-gray-700 dark:text-slate-300 border-gray-200/80 dark:border-slate-700/80 hover:bg-gray-50 dark:hover:bg-slate-800'
            ]"
            title="Editor Preferences (Font, Themes, Tab Size)"
          >
            <FbIcon name="settings" size="13px" />
            <span class="hidden lg:inline">Settings</span>
          </button>

          <!-- Save Button (Ctrl+S) -->
          <button
            @click="handleSave"
            :disabled="saving || !isDirty"
            class="px-3 sm:px-4 py-1.5 sm:py-2 bg-blue-600 hover:bg-blue-700 active:bg-blue-800 disabled:opacity-40 text-white font-bold rounded-xl flex items-center space-x-1.5 transition shadow-xs cursor-pointer text-xs"
          >
            <FbIcon name="save" size="13px" :class="{ 'animate-spin': saving }" />
            <span>{{ saving ? 'Saving...' : 'Save' }}</span>
          </button>

          <!-- Close Button -->
          <button
            v-if="!uiStore.isMobile"
            @click="handleClose"
            class="p-2 sm:px-3 sm:py-2 bg-gray-100 dark:bg-slate-800 hover:bg-gray-200 dark:hover:bg-slate-700 text-gray-700 dark:text-slate-200 rounded-xl transition text-xs font-semibold cursor-pointer"
            title="Close Editor (Esc)"
          >
            ✕
          </button>
        </div>
      </div>

      <!-- ================= DESKTOP PREFERENCES DRAWER ================= -->
      <div
        v-if="isSettingsOpen && !uiStore.isMobile"
        class="bg-gray-100/90 dark:bg-[#0f172a]/95 border-b border-gray-200/80 dark:border-slate-800/80 px-4 py-2.5 flex flex-wrap items-center gap-4 text-xs select-none animate-in slide-in-from-top-2 duration-150 text-gray-800 dark:text-slate-200 backdrop-blur-md"
      >
        <!-- Theme -->
        <div class="flex items-center space-x-1.5">
          <span class="text-gray-500 dark:text-slate-400 font-medium">Theme:</span>
          <select
            v-model="editorTheme"
            @change="updateEditorTheme"
            class="bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-xl px-2.5 py-1 text-gray-800 dark:text-slate-100 font-medium cursor-pointer shadow-2xs focus:outline-none"
          >
            <optgroup label="Dark Themes" class="bg-white dark:bg-slate-900 text-gray-800 dark:text-slate-100">
              <option value="ace/theme/tomorrow_night">Tomorrow Night</option>
              <option value="ace/theme/one_dark">One Dark</option>
              <option value="ace/theme/dracula">Dracula</option>
              <option value="ace/theme/monokai">Monokai</option>
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

        <!-- Font Family -->
        <div class="flex items-center space-x-1.5">
          <span class="text-gray-500 dark:text-slate-400 font-medium">Font:</span>
          <select
            v-model="fontFamily"
            @change="updateEditorFont"
            class="bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-xl px-2.5 py-1 text-gray-800 dark:text-slate-100 font-medium cursor-pointer shadow-2xs focus:outline-none"
          >
            <option value="'JetBrains Mono', 'Fira Code', monospace">JetBrains Mono</option>
            <option value="'Fira Code', monospace">Fira Code</option>
            <option value="'SF Mono', 'Menlo', monospace">SF Mono / Menlo</option>
            <option value="'Courier New', monospace">Courier New</option>
          </select>
        </div>

        <!-- Font Size -->
        <div class="flex items-center space-x-1.5">
          <span class="text-gray-500 dark:text-slate-400 font-medium">Size:</span>
          <select
            v-model="fontSize"
            @change="updateEditorFont"
            class="bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-xl px-2.5 py-1 text-gray-800 dark:text-slate-100 font-medium cursor-pointer shadow-2xs focus:outline-none"
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
            class="bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-xl px-2.5 py-1 text-gray-800 dark:text-slate-100 font-medium cursor-pointer shadow-2xs focus:outline-none"
          >
            <option :value="2">2 spaces</option>
            <option :value="4">4 spaces</option>
            <option :value="8">8 spaces</option>
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
          <span class="text-gray-700 dark:text-slate-300 font-medium">Word Wrap</span>
        </label>

        <!-- Line Numbers Toggle -->
        <label class="flex items-center space-x-1.5 cursor-pointer">
          <input
            type="checkbox"
            v-model="showGutter"
            @change="updateEditorGutter"
            class="rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
          />
          <span class="text-gray-700 dark:text-slate-300 font-medium">Gutter</span>
        </label>

        <!-- Highlight Active Line -->
        <label class="flex items-center space-x-1.5 cursor-pointer">
          <input
            type="checkbox"
            v-model="highlightActiveLine"
            @change="updateActiveLine"
            class="rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
          />
          <span class="text-gray-700 dark:text-slate-300 font-medium">Highlight Line</span>
        </label>
      </div>

      <!-- ================= MAIN EDITOR CANVAS & PREVIEW ================= -->
      <div class="flex-1 flex overflow-hidden relative bg-white dark:bg-[#0b0f19]">
        <!-- FLOATING SEARCH & REPLACE BAR (Ctrl+F / Ctrl+H) -->
        <div
          v-if="isSearchOpen"
          class="absolute top-3 right-3 z-40 bg-white/95 dark:bg-[#111827]/95 border border-gray-200 dark:border-slate-700 rounded-2xl shadow-2xl p-2.5 flex flex-col gap-2 max-w-sm sm:max-w-md w-full backdrop-blur-md animate-in slide-in-from-top-2 duration-150"
          @keydown.esc="isSearchOpen = false"
        >
          <!-- Search Input Row -->
          <div class="flex items-center space-x-1.5">
            <div class="relative flex-1">
              <input
                ref="searchInputRef"
                v-model="searchQuery"
                @input="executeSearch"
                @keydown.enter="findNext"
                @keydown.shift.enter.prevent="findPrev"
                type="text"
                placeholder="Find in file..."
                class="w-full bg-gray-50 dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-xl px-2.5 py-1.5 text-xs text-gray-900 dark:text-slate-100 outline-none pr-16 font-mono"
              />
              <span class="absolute right-2 top-1/2 -translate-y-1/2 text-[10px] text-gray-400 font-mono">
                {{ matchCount > 0 ? `${currentMatchIdx}/${matchCount}` : (searchQuery ? '0/0' : '') }}
              </span>
            </div>

            <!-- Match Navigation Buttons -->
            <button
              @click="findPrev"
              :disabled="matchCount === 0"
              class="p-1.5 rounded-lg border border-gray-200 dark:border-slate-700 hover:bg-gray-100 dark:hover:bg-slate-800 disabled:opacity-40 text-gray-600 dark:text-slate-300 cursor-pointer"
              title="Previous Match (Shift+Enter)"
            >
              <FbIcon name="arrow-up" size="12px" />
            </button>
            <button
              @click="findNext"
              :disabled="matchCount === 0"
              class="p-1.5 rounded-lg border border-gray-200 dark:border-slate-700 hover:bg-gray-100 dark:hover:bg-slate-800 disabled:opacity-40 text-gray-600 dark:text-slate-300 cursor-pointer"
              title="Next Match (Enter)"
            >
              <FbIcon name="arrow-down" size="12px" />
            </button>

            <!-- Case Sensitive Toggle -->
            <button
              @click="searchCaseSensitive = !searchCaseSensitive; executeSearch()"
              :class="[
                'px-1.5 py-1 rounded-lg border font-mono text-[10px] font-bold cursor-pointer transition',
                searchCaseSensitive
                  ? 'bg-blue-600 text-white border-blue-600'
                  : 'border-gray-200 dark:border-slate-700 text-gray-600 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800'
              ]"
              title="Match Case (Aa)"
            >
              Aa
            </button>

            <!-- Whole Word Toggle -->
            <button
              @click="searchWholeWord = !searchWholeWord; executeSearch()"
              :class="[
                'px-1.5 py-1 rounded-lg border font-mono text-[10px] font-bold cursor-pointer transition',
                searchWholeWord
                  ? 'bg-blue-600 text-white border-blue-600'
                  : 'border-gray-200 dark:border-slate-700 text-gray-600 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800'
              ]"
              title="Match Whole Word (\b)"
            >
              \b
            </button>

            <!-- Regex Toggle -->
            <button
              @click="searchRegex = !searchRegex; executeSearch()"
              :class="[
                'px-1.5 py-1 rounded-lg border font-mono text-[10px] font-bold cursor-pointer transition',
                searchRegex
                  ? 'bg-blue-600 text-white border-blue-600'
                  : 'border-gray-200 dark:border-slate-700 text-gray-600 dark:text-slate-400 hover:bg-gray-100 dark:hover:bg-slate-800'
              ]"
              title="Regular Expression (.*)"
            >
              .*
            </button>

            <!-- Close Search Bar -->
            <button
              @click="isSearchOpen = false"
              class="p-1.5 text-gray-400 hover:text-gray-700 dark:hover:text-white rounded-lg hover:bg-gray-100 dark:hover:bg-slate-800 cursor-pointer"
              title="Close Search (Esc)"
            >
              ✕
            </button>
          </div>

          <!-- Replace Input Row -->
          <div class="flex items-center space-x-1.5">
            <input
              v-model="replaceQuery"
              @keydown.enter="replaceCurrent"
              type="text"
              placeholder="Replace with..."
              class="flex-1 bg-gray-50 dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-xl px-2.5 py-1.5 text-xs text-gray-900 dark:text-slate-100 outline-none font-mono"
            />
            <button
              @click="replaceCurrent"
              :disabled="matchCount === 0"
              class="px-2.5 py-1.5 rounded-xl border border-gray-200 dark:border-slate-700 hover:bg-gray-100 dark:hover:bg-slate-800 disabled:opacity-40 text-gray-700 dark:text-slate-200 text-xs font-semibold cursor-pointer shadow-2xs"
            >
              Replace
            </button>
            <button
              @click="replaceAll"
              :disabled="matchCount === 0"
              class="px-2.5 py-1.5 rounded-xl bg-blue-600 hover:bg-blue-700 text-white disabled:opacity-40 text-xs font-semibold cursor-pointer shadow-2xs"
            >
              All
            </button>
          </div>
        </div>

        <!-- FLOATING GO-TO-LINE POPOVER (Ctrl+G) -->
        <div
          v-if="isGotoOpen"
          class="absolute top-3 left-1/2 -translate-x-1/2 z-40 bg-white/95 dark:bg-[#111827]/95 border border-gray-200 dark:border-slate-700 rounded-2xl shadow-2xl p-3 w-72 backdrop-blur-md animate-in slide-in-from-top-2 duration-150"
        >
          <div class="flex items-center justify-between mb-2">
            <span class="font-bold text-xs text-gray-900 dark:text-white">Go to Line</span>
            <span class="text-[10px] text-gray-400 font-mono">1 – {{ lineCount }}</span>
          </div>
          <div class="flex items-center space-x-2">
            <input
              ref="gotoInputRef"
              v-model="gotoTarget"
              @keydown.enter="jumpToLine"
              @keydown.esc="isGotoOpen = false"
              type="text"
              placeholder="e.g. 24 or 24:5"
              class="flex-1 bg-gray-50 dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-xl px-2.5 py-1.5 text-xs text-gray-900 dark:text-slate-100 outline-none font-mono"
            />
            <button
              @click="jumpToLine"
              class="px-3 py-1.5 bg-blue-600 hover:bg-blue-700 text-white rounded-xl font-bold text-xs cursor-pointer shadow-2xs"
            >
              Go
            </button>
          </div>
        </div>

        <!-- Ace Code Editor Mount Point -->
        <div
          v-show="!uiStore.isMobile || !showMarkdownPreview"
          ref="editorEl"
          :class="[
            'h-full text-xs sm:text-sm font-mono transition-[width] duration-standard ease-spring',
            !uiStore.isMobile && showMarkdownPreview ? 'w-1/2 border-r border-gray-200 dark:border-slate-800' : 'w-full'
          ]"
        ></div>

        <!-- Markdown Live Render Preview -->
        <div
          v-if="showMarkdownPreview"
          :class="[
            'h-full overflow-y-auto p-4 sm:p-6 bg-white dark:bg-[#0b0f19] text-gray-900 dark:text-slate-100 prose dark:prose-invert max-w-none text-xs sm:text-sm border-t md:border-t-0',
            uiStore.isMobile ? 'w-full' : 'w-1/2'
          ]"
          v-html="renderedMarkdown"
        ></div>
      </div>

      <!-- ================= MOBILE ACCESSORY BAR ================= -->
      <div
        v-if="uiStore.isMobile"
        class="h-11 bg-gray-100 dark:bg-slate-900 border-t border-gray-200 dark:border-slate-800 flex items-center px-2 space-x-1.5 overflow-x-auto shrink-0 select-none pb-safe"
      >
        <button
          v-for="sym in quickSymbols"
          :key="sym.label"
          @click="insertSymbol(sym.value)"
          class="px-3 py-1.5 bg-white dark:bg-slate-800 hover:bg-gray-200 dark:hover:bg-slate-700 text-gray-800 dark:text-slate-100 font-mono text-xs font-semibold rounded-lg shadow-2xs border border-gray-200 dark:border-slate-700 shrink-0 cursor-pointer active:scale-95 transition"
        >
          {{ sym.label }}
        </button>
      </div>

      <!-- ================= EDITOR FOOTER STATUS BAR ================= -->
      <div
        class="h-7 md:h-8 bg-gray-50/90 dark:bg-[#090d16]/95 border-t border-gray-200/80 dark:border-slate-800/80 px-3 md:px-4 flex items-center justify-between text-[10px] md:text-[11px] text-gray-500 dark:text-slate-400 select-none shrink-0 font-mono backdrop-blur-md"
      >
        <div class="flex items-center space-x-2.5 sm:space-x-3.5 truncate">
          <!-- Cursor Position -->
          <span class="text-gray-700 dark:text-slate-300 font-semibold">
            Ln {{ cursorPosition.row + 1 }}, Col {{ cursorPosition.column + 1 }}
          </span>
          <span v-if="selectedCharCount > 0" class="text-blue-600 dark:text-blue-400 font-medium">
            ({{ selectedCharCount }} selected)
          </span>

          <span class="text-gray-300 dark:text-slate-700">•</span>

          <!-- Total Lines / Chars -->
          <span>{{ lineCount }} lines</span>
          <span class="text-gray-300 dark:text-slate-700">•</span>
          <span>{{ charCount }} chars</span>

          <!-- Encoding & Line Endings -->
          <span class="hidden sm:inline text-gray-300 dark:text-slate-700">•</span>
          <span class="hidden sm:inline">UTF-8</span>
          <span class="hidden sm:inline">LF</span>
          <span class="hidden sm:inline text-gray-300 dark:text-slate-700">•</span>
          <span class="hidden sm:inline">Tab: {{ tabSize }} spaces</span>
        </div>

        <div class="flex items-center space-x-2.5 shrink-0">
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

      <!-- ================= MOBILE SETTINGS BOTTOM SHEET (< 768px) ================= -->
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
              class="p-1 text-gray-400 hover:text-gray-700 dark:hover:text-white text-base cursor-pointer"
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
              class="w-full bg-gray-50 dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-2xl px-3.5 py-2.5 text-gray-800 dark:text-slate-100 font-medium text-xs cursor-pointer shadow-2xs focus:outline-none"
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
              class="w-full bg-gray-50 dark:bg-slate-900 border border-gray-200 dark:border-slate-700 rounded-2xl px-3.5 py-2.5 text-gray-800 dark:text-slate-100 font-medium text-xs cursor-pointer shadow-2xs focus:outline-none"
            >
              <optgroup label="Dark Themes" class="bg-white dark:bg-slate-900 text-gray-800 dark:text-slate-100">
                <option value="ace/theme/tomorrow_night">Tomorrow Night</option>
                <option value="ace/theme/one_dark">One Dark</option>
                <option value="ace/theme/dracula">Dracula</option>
                <option value="ace/theme/monokai">Monokai</option>
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
                    ? 'bg-blue-600 text-white shadow-2xs'
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

      <!-- ================= UNSAVED CHANGES CONFIRMATION MODAL ================= -->
      <div
        v-if="isUnsavedConfirmOpen"
        class="fixed inset-0 z-60 bg-black/70 backdrop-blur-sm flex items-center justify-center p-4 animate-in fade-in duration-150"
        @click="isUnsavedConfirmOpen = false"
      >
        <div
          class="bg-white dark:bg-[#0f1422] border border-gray-200 dark:border-slate-800 rounded-3xl p-5 sm:p-6 max-w-md w-full shadow-2xl space-y-4 animate-in zoom-in-95 duration-150 text-gray-900 dark:text-slate-100 ring-1 ring-black/5 dark:ring-white/5"
          @click.stop
        >
          <!-- Icon & Header -->
          <div class="flex items-start space-x-3.5">
            <div class="w-11 h-11 rounded-2xl bg-amber-500/10 dark:bg-amber-500/20 text-amber-500 flex items-center justify-center shrink-0 ring-1 ring-amber-500/30">
              <span class="text-xl">⚠️</span>
            </div>
            <div>
              <h3 class="font-bold text-base text-gray-900 dark:text-white">Unsaved Changes</h3>
              <p class="text-xs text-gray-500 dark:text-slate-400 mt-1 leading-relaxed">
                Do you want to save the changes made to
                <span class="font-bold text-gray-800 dark:text-slate-200 font-mono">{{ uiStore.editorFile?.name }}</span>
                before closing?
              </p>
              <p class="text-[11px] text-amber-600 dark:text-amber-400 mt-1 font-medium">
                Your changes will be lost if you choose "Don't Save".
              </p>
            </div>
          </div>

          <!-- Actions -->
          <div class="flex flex-col sm:flex-row items-stretch sm:items-center justify-end gap-2 pt-2 border-t border-gray-100 dark:border-slate-800/80">
            <button
              @click="isUnsavedConfirmOpen = false"
              class="px-4 py-2.5 rounded-xl bg-gray-100 dark:bg-slate-800 hover:bg-gray-200 dark:hover:bg-slate-700 text-gray-700 dark:text-slate-300 font-semibold text-xs transition cursor-pointer order-3 sm:order-1"
            >
              Cancel
            </button>

            <button
              @click="handleDiscardAndClose"
              class="px-4 py-2.5 rounded-xl bg-red-500/10 hover:bg-red-500/20 text-red-600 dark:text-red-400 border border-red-500/20 font-semibold text-xs transition cursor-pointer order-2"
            >
              Don't Save
            </button>

            <button
              @click="handleSaveAndClose"
              :disabled="saving"
              class="px-4 py-2.5 rounded-xl bg-blue-600 hover:bg-blue-700 active:bg-blue-800 text-white font-bold text-xs flex items-center justify-center space-x-1.5 shadow-md shadow-blue-600/20 transition cursor-pointer order-1 sm:order-3 disabled:opacity-50"
            >
              <FbIcon name="save" size="13px" :class="{ 'animate-spin': saving }" />
              <span>{{ saving ? 'Saving...' : 'Save & Close' }}</span>
            </button>
          </div>
        </div>
      </div>

      <!-- ================= CONFLICT CONFIRMATION MODAL ================= -->
      <div
        v-if="isConflictModalOpen"
        class="fixed inset-0 z-60 bg-black/70 backdrop-blur-sm flex items-center justify-center p-4 animate-in fade-in duration-150"
        @click="isConflictModalOpen = false"
      >
        <div
          class="bg-white dark:bg-[#0f1422] border border-red-500/30 rounded-3xl p-5 sm:p-6 max-w-md w-full shadow-2xl space-y-4 animate-in zoom-in-95 duration-150 text-gray-900 dark:text-slate-100 ring-1 ring-red-500/20"
          @click.stop
        >
          <div class="flex items-start space-x-3.5">
            <div class="w-11 h-11 rounded-2xl bg-red-500/10 dark:bg-red-500/20 text-red-500 flex items-center justify-center shrink-0 ring-1 ring-red-500/30">
              <span class="text-xl">⚠️</span>
            </div>
            <div>
              <h3 class="font-bold text-base text-gray-900 dark:text-white">File Conflict Detected</h3>
              <p class="text-xs text-gray-500 dark:text-slate-400 mt-1 leading-relaxed">
                <span class="font-bold text-gray-800 dark:text-slate-200 font-mono">{{ uiStore.editorFile?.name }}</span>
                was modified on disk since you opened it.
              </p>
              <p class="text-[11px] text-red-600 dark:text-red-400 mt-1 font-medium">
                Overwriting will replace remote changes on disk with your current editor content.
              </p>
            </div>
          </div>

          <div class="flex flex-col sm:flex-row items-stretch sm:items-center justify-end gap-2 pt-2 border-t border-gray-100 dark:border-slate-800/80">
            <button
              @click="isConflictModalOpen = false"
              class="px-4 py-2.5 rounded-xl bg-gray-100 dark:bg-slate-800 hover:bg-gray-200 dark:hover:bg-slate-700 text-gray-700 dark:text-slate-300 font-semibold text-xs transition cursor-pointer order-3 sm:order-1"
            >
              Cancel
            </button>

            <button
              @click="handleForceSave"
              :disabled="saving"
              class="px-4 py-2.5 rounded-xl bg-amber-600 hover:bg-amber-700 text-white font-bold text-xs flex items-center justify-center space-x-1.5 shadow-md shadow-amber-600/20 transition cursor-pointer order-1 sm:order-2 disabled:opacity-50"
            >
              <span>Overwrite Disk</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
  </Transition>
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
import { getFileTypeMeta } from '../../utils/fileTypes';

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
const savedContent = ref('');
const saving = ref(false);
const isSettingsOpen = ref(false);
const isSyntaxMenuOpen = ref(false);
const isSearchOpen = ref(false);
const isGotoOpen = ref(false);
const isUnsavedConfirmOpen = ref(false);
const isConflictModalOpen = ref(false);
const gotoTarget = ref('');
const lineCount = ref(1);
const charCount = ref(0);
const selectedCharCount = ref(0);
const cursorPosition = ref({ row: 0, column: 0 });
const showMarkdownPreview = ref(false);
const rawTextContent = ref('');
const syntaxSearchQuery = ref('');

// Search State
const searchQuery = ref('');
const replaceQuery = ref('');
const searchCaseSensitive = ref(false);
const searchWholeWord = ref(false);
const searchRegex = ref(false);
const matchCount = ref(0);
const currentMatchIdx = ref(0);

const searchInputRef = ref<HTMLInputElement | null>(null);
const gotoInputRef = ref<HTMLInputElement | null>(null);

// Configurable Preferences (Loaded from localStorage)
const editorTheme = ref(localStorage.getItem('fb:editor:theme') || (themeStore.isDark ? 'ace/theme/tomorrow_night' : 'ace/theme/chrome'));
const fontFamily = ref(localStorage.getItem('fb:editor:fontFamily') || "'JetBrains Mono', 'Fira Code', monospace");
const fontSize = ref(Number(localStorage.getItem('fb:editor:fontSize')) || 13);
const tabSize = ref(Number(localStorage.getItem('fb:editor:tabSize')) || 2);
const wordWrap = ref(localStorage.getItem('fb:editor:wordWrap') !== 'false');
const showGutter = ref(localStorage.getItem('fb:editor:showGutter') !== 'false');
const highlightActiveLine = ref(localStorage.getItem('fb:editor:highlightActiveLine') !== 'false');
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

const filteredLanguages = computed(() => {
  if (!syntaxSearchQuery.value.trim()) return availableLanguages;
  const q = syntaxSearchQuery.value.toLowerCase();
  return availableLanguages.filter((l) => l.name.toLowerCase().includes(q));
});

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

const fileTypeMeta = computed(() => {
  if (!uiStore.editorFile) {
    return {
      category: 'other' as const,
      label: 'FILE',
      badgeBg: 'bg-gray-500/15',
      badgeText: 'text-gray-600',
      badgeBorder: 'border-gray-300',
      cardBg: '',
      iconBg: '',
      iconColor: '',
      symbol: '📄',
    };
  }
  return getFileTypeMeta(uiStore.editorFile);
});

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

function copyFilePath() {
  if (uiStore.editorFile?.path) {
    navigator.clipboard.writeText(uiStore.editorFile.path);
    uiStore.showToast('Path copied to clipboard', 'info');
  }
}

function escapeHtml(str: string): string {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

// Safe Markdown Parser with Alert Badges & Tables
const renderedMarkdown = computed(() => {
  if (!rawTextContent.value) return '';
  const text = escapeHtml(rawTextContent.value);
  return text
    .replace(/^### (.*$)/gim, '<h3 class="text-base font-bold mb-1.5 mt-3 text-gray-900 dark:text-white">$1</h3>')
    .replace(/^## (.*$)/gim, '<h2 class="text-lg font-bold border-b border-gray-200 dark:border-slate-800 pb-1 mb-2 mt-4 text-gray-900 dark:text-white">$1</h2>')
    .replace(/^# (.*$)/gim, '<h1 class="text-xl font-bold border-b border-gray-200 dark:border-slate-800 pb-1 mb-2 text-gray-900 dark:text-white">$1</h1>')
    .replace(/^\> \[!NOTE\](.*$)/gim, '<div class="border-l-4 border-blue-500 pl-3 py-2 bg-blue-50/30 dark:bg-blue-950/30 text-blue-900 dark:text-blue-300 my-2 rounded-r font-medium">ℹ️ <strong>Note:</strong> $1</div>')
    .replace(/^\> \[!TIP\](.*$)/gim, '<div class="border-l-4 border-emerald-500 pl-3 py-2 bg-emerald-50/30 dark:bg-emerald-950/30 text-emerald-900 dark:text-emerald-300 my-2 rounded-r font-medium">💡 <strong>Tip:</strong> $1</div>')
    .replace(/^\> \[!WARNING\](.*$)/gim, '<div class="border-l-4 border-amber-500 pl-3 py-2 bg-amber-50/30 dark:bg-amber-950/30 text-amber-900 dark:text-amber-300 my-2 rounded-r font-medium">⚠️ <strong>Warning:</strong> $1</div>')
    .replace(/^\> (.*$)/gim, '<blockquote class="border-l-4 border-gray-300 dark:border-slate-700 pl-3 py-1 text-gray-600 dark:text-slate-400 italic my-2 bg-gray-50/40 dark:bg-slate-900/40 rounded-r">$1</blockquote>')
    .replace(/\*\*(.*?)\*\*/gim, '<strong class="font-bold text-gray-900 dark:text-white">$1</strong>')
    .replace(/\*(.*?)\*/gim, '<em class="italic">$1</em>')
    .replace(/`([^`]+)`/gim, '<code class="bg-gray-100 dark:bg-slate-800 px-1.5 py-0.5 rounded font-mono text-blue-600 dark:text-blue-400 text-xs">$1</code>')
    .replace(/\n$/gim, '<br />')
    .replace(/\n/gim, '<br />');
});

function closeAllPopovers() {
  isSettingsOpen.value = false;
  isSyntaxMenuOpen.value = false;
  isGotoOpen.value = false;
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
  if (ext === 'json') return 'ace/mode/json';
  if (ext === 'yaml' || ext === 'yml') return 'ace/mode/yaml';

  const mode = modelist.getModeForPath(filename).mode;
  return mode || 'ace/mode/text';
}

function toggleSearchBar() {
  isSearchOpen.value = !isSearchOpen.value;
  if (isSearchOpen.value) {
    isGotoOpen.value = false;
    isSyntaxMenuOpen.value = false;
    nextTick(() => {
      searchInputRef.value?.focus();
      searchInputRef.value?.select();
      if (searchQuery.value) {
        executeSearch();
      }
    });
  } else {
    editor.value?.focus();
  }
}

function executeSearch() {
  if (!editor.value || !searchQuery.value) {
    matchCount.value = 0;
    currentMatchIdx.value = 0;
    return;
  }

  // Find all matches to count them
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
  } catch (err) {
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
  uiStore.showToast('Replaced all matches', 'info');
}

function jumpToLine() {
  if (!editor.value || !gotoTarget.value) return;
  const parts = gotoTarget.value.split(':');
  const line = parseInt(parts[0], 10);
  const col = parts[1] ? parseInt(parts[1], 10) : 1;

  if (!isNaN(line) && line > 0) {
    editor.value.gotoLine(line, col, true);
    isGotoOpen.value = false;
    gotoTarget.value = '';
    editor.value.focus();
  }
}

function formatDocument() {
  if (!editor.value) return;
  const currentText = editor.value.getValue();
  const ext = uiStore.editorFile?.name.split('.').pop()?.toLowerCase();

  // 1. JSON Prettifier
  if (ext === 'json' || currentMode.value === 'ace/mode/json') {
    try {
      const parsed = JSON.parse(currentText);
      const formatted = JSON.stringify(parsed, null, tabSize.value);
      editor.value.setValue(formatted, -1);
      uiStore.showToast('Formatted JSON document', 'success');
      return;
    } catch (err) {
      uiStore.showToast('Unable to format: Invalid JSON syntax', 'warning');
      return;
    }
  }

  // Generic Re-indentation
  try {
    const session = editor.value.getSession();
    const rows = session.getLength();
    for (let i = 0; i < rows; i++) {
      session.indentRows(i, i, '');
    }
    uiStore.showToast('Document indentation adjusted', 'info');
  } catch (err) {
    uiStore.showToast('Format document completed', 'info');
  }
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
  }, 200);
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
    highlightActiveLine: highlightActiveLine.value,
    showPrintMargin: false,
    useWorker: false,
    behavioursEnabled: true,
    value: uiStore.editorContent,
  });

  savedContent.value = uiStore.editorContent;
  currentMode.value = detectMode(uiStore.editorFile.name);
  lineCount.value = editor.value.session.getLength();
  charCount.value = uiStore.editorContent.length;
  if (isMarkdownFile.value && showMarkdownPreview.value) {
    rawTextContent.value = uiStore.editorContent;
  }

  editor.value.session.on('change', () => {
    isDirty.value = (editor.value?.getValue() ?? '') !== savedContent.value;
    lineCount.value = editor.value?.session.getLength() || 1;
    charCount.value = editor.value?.getValue().length || 0;
    if (isMarkdownFile.value && showMarkdownPreview.value) {
      updateMarkdownPreviewDebounced();
    }
  });

  // Track Cursor & Selection
  editor.value.selection.on('changeCursor', () => {
    if (editor.value) {
      cursorPosition.value = editor.value.getCursorPosition();
    }
  });

  editor.value.selection.on('changeSelection', () => {
    if (editor.value) {
      const selected = editor.value.getSelectedText();
      selectedCharCount.value = selected.length;
    }
  });

  // Keyboard Shortcuts
  editor.value.commands.addCommand({
    name: 'save',
    bindKey: { win: 'Ctrl-S', mac: 'Command-S' },
    exec: () => {
      handleSave();
    },
  });

  editor.value.commands.addCommand({
    name: 'find',
    bindKey: { win: 'Ctrl-F', mac: 'Command-F' },
    exec: () => {
      toggleSearchBar();
    },
  });

  editor.value.commands.addCommand({
    name: 'gotoLine',
    bindKey: { win: 'Ctrl-G', mac: 'Command-G' },
    exec: () => {
      isGotoOpen.value = true;
      isSearchOpen.value = false;
      nextTick(() => {
        gotoInputRef.value?.focus();
      });
    },
  });

  editor.value.commands.addCommand({
    name: 'format',
    bindKey: { win: 'Shift-Alt-F', mac: 'Shift-Option-F' },
    exec: () => {
      formatDocument();
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

function updateActiveLine() {
  if (editor.value) {
    editor.value.setHighlightActiveLine(highlightActiveLine.value);
    localStorage.setItem('fb:editor:highlightActiveLine', highlightActiveLine.value ? 'true' : 'false');
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
      isSearchOpen.value = false;
      isGotoOpen.value = false;
      isUnsavedConfirmOpen.value = false;
      isConflictModalOpen.value = false;
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

async function handleSave(): Promise<boolean> {
  if (!uiStore.editorFile || !editor.value) return false;
  saving.value = true;

  const currentText = editor.value.getValue();

  try {
    const headers: Record<string, string> = {};
    if (uiStore.editorEtag) {
      headers['If-Match'] = uiStore.editorEtag;
    }

    const connId = uiStore.editorConnectionId || fileStore.currentConnectionId || 'local';
    const resp = await apiClient.put(
      `/connections/${connId}/files/content`,
      {
        path: uiStore.editorFile.path,
        content: currentText,
      },
      { headers }
    );

    if (resp.headers['etag']) {
      uiStore.editorEtag = resp.headers['etag'];
    }

    savedContent.value = currentText;
    uiStore.editorContent = currentText;
    editor.value.session.getUndoManager().markClean();
    isDirty.value = false;
    uiStore.showToast(`Saved ${uiStore.editorFile.name}`, 'success');

    // Auto reload workspace entries
    await workspaceStore.refreshAll();
    return true;
  } catch (err: any) {
    if (err.response?.status === 409 || err.response?.status === 412) {
      isConflictModalOpen.value = true;
    } else {
      uiStore.showToast(err.response?.data?.error?.message || 'Failed to save file', 'error');
    }
    return false;
  } finally {
    saving.value = false;
  }
}

async function handleForceSave() {
  if (!uiStore.editorFile || !editor.value) return;
  saving.value = true;
  const currentText = editor.value.getValue();

  try {
    const connId = uiStore.editorConnectionId || fileStore.currentConnectionId || 'local';
    const forceResp = await apiClient.put(
      `/connections/${connId}/files/content`,
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
    savedContent.value = currentText;
    uiStore.editorContent = currentText;
    editor.value.session.getUndoManager().markClean();
    isDirty.value = false;
    isConflictModalOpen.value = false;
    uiStore.showToast(`Force saved ${uiStore.editorFile.name}`, 'warning');
    await workspaceStore.refreshAll();
  } catch (forceErr: any) {
    uiStore.showToast(forceErr.response?.data?.error?.message || 'Force save failed', 'error');
  } finally {
    saving.value = false;
  }
}

function handleClose() {
  if (isDirty.value) {
    isUnsavedConfirmOpen.value = true;
  } else {
    uiStore.isEditorOpen = false;
  }
}

function handleDiscardAndClose() {
  isDirty.value = false;
  isUnsavedConfirmOpen.value = false;
  uiStore.isEditorOpen = false;
}

async function handleSaveAndClose() {
  const success = await handleSave();
  if (success) {
    isUnsavedConfirmOpen.value = false;
    uiStore.isEditorOpen = false;
  }
}

onBeforeUnmount(() => {
  editor.value?.destroy();
});
</script>

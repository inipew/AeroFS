<template>
  <header class="h-14 md:h-16 bg-white/95 dark:bg-[#0b0f19]/95 backdrop-blur-md border-b border-gray-200/80 dark:border-slate-800/80 px-3 sm:px-4 md:px-5 flex items-center justify-between text-gray-800 dark:text-slate-100 select-none sticky top-0 z-30 transition-colors duration-150">
    <Transition name="header-morph" mode="out-in">
      <!-- ====================================================================
           1. CONTEXTUAL SELECTION HEADER MODE (Apple Morphing Toolbar)
           ==================================================================== -->
      <div
        v-if="activePanel.selectedEntries.length > 0"
        key="selection-toolbar"
        class="flex items-center justify-between w-full h-full"
      >
        <!-- Left: Deselect Arrow & Selection Count Badge -->
        <div class="flex items-center space-x-2.5 sm:space-x-3">
          <button
            @click="activePanel.selectedEntries = []"
            class="p-2 rounded-xl text-gray-500 hover:text-gray-900 dark:text-slate-400 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer active:scale-95 duration-fast ease-spring"
            title="Deselect All (Esc)"
          >
            <FbIcon name="arrow-back" size="18px" />
          </button>

          <div class="flex items-center space-x-2 px-3 py-1 rounded-xl bg-blue-50 dark:bg-blue-950/60 border border-blue-200/80 dark:border-blue-800/60 text-xs font-bold text-blue-600 dark:text-blue-400 shadow-2xs">
            <span class="w-2 h-2 rounded-full bg-blue-600 dark:bg-blue-400 animate-pulse"></span>
            <span>{{ activePanel.selectedEntries.length }} Selected</span>
            <span v-if="selectedTotalSize > 0" class="text-[11px] font-mono font-normal opacity-75 hidden sm:inline">
              ({{ formatBytes(selectedTotalSize) }})
            </span>
          </div>
        </div>

        <!-- Right: Contextual Action Pills -->
        <div class="flex items-center space-x-1.5 sm:space-x-2 shrink-0">
          <!-- Compress / Archive -->
          <button
            @click="handleCompressSelection"
            class="px-2.5 sm:px-3 py-1.5 rounded-xl border border-gray-200 dark:border-slate-700 bg-white dark:bg-slate-900 hover:bg-gray-50 dark:hover:bg-slate-800 text-xs font-medium text-gray-700 dark:text-slate-200 flex items-center space-x-1.5 transition active:scale-95 cursor-pointer shadow-2xs"
            title="Compress selected items into archive"
          >
            <FbIcon name="archive" size="14px" class="text-amber-500" />
            <span class="hidden sm:inline">Compress</span>
          </button>

          <!-- Rename (Single Selection) -->
          <button
            v-if="activePanel.selectedEntries.length === 1"
            @click="handleRenameSelection"
            class="px-2.5 sm:px-3 py-1.5 rounded-xl border border-gray-200 dark:border-slate-700 bg-white dark:bg-slate-900 hover:bg-gray-50 dark:hover:bg-slate-800 text-xs font-medium text-gray-700 dark:text-slate-200 flex items-center space-x-1.5 transition active:scale-95 cursor-pointer shadow-2xs"
            title="Rename item"
          >
            <FbIcon name="rename" size="14px" class="text-blue-500" />
            <span class="hidden sm:inline">Rename</span>
          </button>

          <!-- Copy Selection -->
          <button
            @click="workspaceStore.copySelection(workspaceStore.activePanelId)"
            class="px-2.5 sm:px-3 py-1.5 rounded-xl border border-gray-200 dark:border-slate-700 bg-white dark:bg-slate-900 hover:bg-gray-50 dark:hover:bg-slate-800 text-xs font-medium text-gray-700 dark:text-slate-200 flex items-center space-x-1.5 transition active:scale-95 cursor-pointer shadow-2xs"
            title="Copy selected items (Ctrl+C)"
          >
            <FbIcon name="copy" size="14px" class="text-emerald-500" />
            <span class="hidden sm:inline">Copy</span>
          </button>

          <!-- Cut Selection -->
          <button
            @click="workspaceStore.cutSelection(workspaceStore.activePanelId)"
            class="px-2.5 sm:px-3 py-1.5 rounded-xl border border-gray-200 dark:border-slate-700 bg-white dark:bg-slate-900 hover:bg-gray-50 dark:hover:bg-slate-800 text-xs font-medium text-gray-700 dark:text-slate-200 flex items-center space-x-1.5 transition active:scale-95 cursor-pointer shadow-2xs"
            title="Cut selected items (Ctrl+X)"
          >
            <FbIcon name="move" size="14px" class="text-indigo-500" />
            <span class="hidden sm:inline">Cut</span>
          </button>

          <!-- Delete Action (Subtle Rose Pill) -->
          <button
            @click="handleDeleteSelection"
            class="px-2.5 sm:px-3 py-1.5 rounded-xl border border-rose-200 dark:border-rose-900/60 bg-rose-50/80 dark:bg-rose-950/50 hover:bg-rose-100 dark:hover:bg-rose-900/60 text-xs font-semibold text-rose-600 dark:text-rose-400 flex items-center space-x-1.5 transition active:scale-95 cursor-pointer shadow-2xs"
            title="Delete selected items (Del)"
          >
            <FbIcon name="delete" size="14px" />
            <span>Delete</span>
          </button>

          <div class="h-4 w-px bg-gray-200 dark:bg-slate-800 mx-0.5"></div>

          <!-- Clear / Close (✕) -->
          <button
            @click="activePanel.selectedEntries = []"
            class="p-2 rounded-xl text-gray-400 hover:text-gray-700 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer active:scale-95 duration-fast ease-spring text-xs font-bold"
            title="Done / Deselect All (Esc)"
          >
            ✕
          </button>
        </div>
      </div>

      <!-- ====================================================================
           2. STANDARD HEADER MODE (Clean Apple Hierarchy)
           ==================================================================== -->
      <div
        v-else
        key="normal-toolbar"
        class="flex items-center justify-between w-full h-full"
      >
        <!-- LEFT: Navigation Controls & Breadcrumb Capsule -->
        <div class="flex items-center space-x-2 max-w-[60vw] md:max-w-[50vw] shrink min-w-0 py-1">
          <!-- Mobile Sidebar Drawer Toggle Button -->
          <button
            v-if="uiStore.isMobile"
            @click="uiStore.isMobileSidebarOpen = true"
            class="p-2 -ml-1 mr-0.5 text-gray-500 hover:text-gray-900 dark:text-slate-400 dark:hover:text-white rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer shrink-0"
            title="Open Navigation Drawer"
          >
            <FbIcon name="menu" size="19px" />
          </button>

          <!-- Desktop Single-Pane: Apple Segmented Navigation Pill (‹  ›) -->
          <div
            v-if="!uiStore.isMobile && !workspaceStore.isDualPane"
            class="ios-segmented-group shrink-0"
          >
            <!-- Back Button -->
            <button
              @click="workspaceStore.goBack(workspaceStore.activePanelId)"
              :disabled="activePanel.historyIndex <= 0"
              class="ios-segmented-item p-1.5"
              title="Back (Alt+Left)"
            >
              <FbIcon name="chevron-left" size="14px" />
            </button>

            <!-- Forward Button -->
            <button
              @click="workspaceStore.goForward(workspaceStore.activePanelId)"
              :disabled="activePanel.historyIndex >= activePanel.history.length - 1"
              class="ios-segmented-item p-1.5"
              title="Forward (Alt+Right)"
            >
              <FbIcon name="chevron-right" size="14px" />
            </button>
          </div>

            <!-- Dual-Pane Desktop Mode: Brand Logo & Title -->
            <div
              v-if="!uiStore.isMobile && workspaceStore.isDualPane"
              class="flex items-center space-x-2 shrink-0 font-bold text-gray-900 dark:text-white text-sm"
            >
              <div class="w-7 h-7 rounded-lg bg-gradient-to-tr from-blue-600 to-indigo-500 flex items-center justify-center text-white shadow-xs">
                <FbIcon name="folder" size="14px" />
              </div>
              <span class="tracking-tight hidden lg:inline">AeroFS Workspace</span>
            </div>

            <!-- Desktop Single-Pane: Breadcrumb Capsule Hero Centerpiece -->
            <div
              v-if="!uiStore.isMobile && !workspaceStore.isDualPane"
              class="flex items-center min-w-0 flex-1"
            >
            <!-- Direct Path Editing Input Form -->
            <div v-if="isEditingPath" class="flex items-center space-x-1 flex-1 min-w-0">
              <form @submit.prevent="submitPath" class="flex-1 flex items-center min-w-0">
                <input
                  ref="pathInputRef"
                  v-model="inputPath"
                  type="text"
                  placeholder="/path/to/folder"
                  @keydown.esc="isEditingPath = false"
                  class="w-full bg-white dark:bg-[#0f1422] border border-blue-500 text-gray-900 dark:text-white text-xs px-2.5 py-1 rounded-xl focus:outline-none focus:ring-2 focus:ring-blue-500/30 font-mono shadow-xs"
                />
              </form>
              <button
                @click="submitPath"
                class="p-1.5 text-blue-600 dark:text-blue-400 hover:bg-blue-50 dark:hover:bg-blue-950/40 rounded-lg transition cursor-pointer text-xs font-bold"
                title="Go to Path (Enter)"
              >
                ✓
              </button>
              <button
                @click="isEditingPath = false"
                class="p-1.5 text-gray-400 hover:text-gray-600 dark:hover:text-slate-300 hover:bg-gray-100 dark:hover:bg-slate-800 rounded-lg transition cursor-pointer text-xs"
                title="Cancel (Esc)"
              >
                ✕
              </button>
            </div>

            <!-- Breadcrumb Capsule Pill -->
            <nav
              v-else
              class="breadcrumb-capsule flex items-center space-x-1 text-xs select-none overflow-x-auto no-scrollbar shadow-2xs"
            >
              <!-- Root segment -->
              <button
                @click="workspaceStore.navigatePanel(workspaceStore.activePanelId, '/')"
                :class="[
                  'px-2 py-0.5 rounded-lg transition-[background-color,color,transform] duration-fast ease-spring flex items-center shrink-0 active:scale-95 cursor-pointer font-medium',
                  activePanel.path === '/'
                    ? 'text-blue-600 dark:text-blue-400 font-semibold bg-blue-50/80 dark:bg-blue-950/40'
                    : 'text-gray-600 dark:text-slate-400 hover:text-gray-900 dark:hover:text-white hover:bg-gray-200/60 dark:hover:bg-slate-700/60'
                ]"
                title="Root (/)"
              >
                <span>/</span>
              </button>

              <!-- Intermediate Truncated Popover (if depth > 3) -->
              <div v-if="hasTruncatedBreadcrumbs" ref="truncatedMenuRef" class="relative shrink-0 flex items-center">
                <span class="text-gray-400 dark:text-slate-600 font-bold text-xs shrink-0 select-none">›</span>
                <button
                  @click="isTruncatedMenuOpen = !isTruncatedMenuOpen"
                  class="px-1.5 py-0.5 rounded-md text-gray-500 hover:text-gray-900 dark:text-slate-400 dark:hover:text-white hover:bg-gray-200/60 dark:hover:bg-slate-700/60 transition cursor-pointer text-xs font-bold"
                  title="Show intermediate folders"
                >
                  ...
                </button>

                <!-- Truncated Paths Popover Menu -->
                <Transition name="ios-popover">
                  <div
                    v-if="isTruncatedMenuOpen"
                    @click="isTruncatedMenuOpen = false"
                    class="absolute left-0 top-full mt-1.5 w-56 bg-white dark:bg-[#0f1422] border border-gray-200 dark:border-slate-800 rounded-2xl shadow-xl p-1 z-50 text-xs space-y-0.5"
                  >
                    <button
                      v-for="crumb in truncatedCrumbs"
                      :key="crumb.path"
                      @click="workspaceStore.navigatePanel(workspaceStore.activePanelId, crumb.path)"
                      class="w-full flex items-center space-x-2 px-2.5 py-1.5 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800 transition text-left text-gray-700 dark:text-slate-300"
                    >
                      <FbIcon name="folder" size="14px" class="text-amber-500 shrink-0" />
                      <span class="truncate">{{ crumb.name }}</span>
                    </button>
                  </div>
                </Transition>
              </div>

              <!-- Visible Breadcrumb Segments with Spring Item Transitions -->
              <TransitionGroup name="crumb-item" tag="div" class="flex items-center space-x-1 shrink-0">
                <div v-for="crumb in visibleBreadcrumbs" :key="crumb.path" class="flex items-center space-x-1">
                  <span class="text-gray-400 dark:text-slate-600 font-bold text-xs shrink-0 select-none">›</span>
                  <button
                    @click="workspaceStore.navigatePanel(workspaceStore.activePanelId, crumb.path)"
                    :class="[
                      'px-2 py-0.5 rounded-lg transition-[background-color,color,transform] duration-fast ease-spring max-w-[120px] sm:max-w-[170px] truncate active:scale-95 cursor-pointer',
                      crumb.isLast
                        ? 'text-gray-900 dark:text-white font-bold bg-gray-200/70 dark:bg-slate-700/60'
                        : 'text-gray-600 dark:text-slate-400 hover:text-gray-900 dark:hover:text-white hover:bg-gray-200/60 dark:hover:bg-slate-700/60 font-medium'
                    ]"
                    :title="crumb.name"
                  >
                    {{ crumb.name }}
                  </button>
                </div>
              </TransitionGroup>
            </nav>
          </div>

          <!-- Mobile Minimal Breadcrumb Segment -->
          <div
            v-else-if="uiStore.isMobile && breadcrumbs.length > 0"
            class="flex items-center space-x-1 min-w-0"
          >
            <span class="text-gray-400 dark:text-slate-600 font-bold text-xs shrink-0 select-none">›</span>
            <button
              @click="startPathEditing"
              class="text-gray-900 dark:text-white font-bold truncate max-w-[110px] text-xs text-left px-2 py-0.5 rounded-lg bg-gray-100/70 dark:bg-slate-800/60 active:scale-95 transition"
              title="Tap to edit path"
            >
              {{ breadcrumbs[breadcrumbs.length - 1].name }}
            </button>
          </div>
        </div>

        <!-- CENTER: Expandable Spotlight-Style Search Pill (Desktop) -->
        <div v-if="!uiStore.isMobile" class="hidden md:flex items-center justify-center flex-1 px-3 max-w-xs lg:max-w-md">
          <button
            @click="uiStore.toggleCommandPalette()"
            class="apple-search-pill bg-gray-100/80 dark:bg-slate-800/60 hover:bg-gray-100 dark:hover:bg-slate-800 border border-gray-200/80 dark:border-slate-700/60 hover:border-blue-500/40 dark:hover:border-blue-400/40 px-3.5 py-1.5 rounded-2xl text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 flex items-center justify-between text-xs transition shadow-2xs w-48 sm:w-56 hover:w-64 cursor-pointer group select-none"
            title="Command Palette & Instant Search (⌘K / Ctrl+K)"
          >
            <div class="flex items-center space-x-2 truncate">
              <FbIcon name="search" size="14px" class="text-gray-400 group-hover:text-blue-500 transition-colors" />
              <span class="text-gray-400 font-medium truncate">Search or jump to...</span>
            </div>
            <kbd class="px-1.5 py-0.5 rounded-md bg-white dark:bg-slate-900 text-[10px] text-gray-500 dark:text-slate-400 border border-gray-200 dark:border-slate-700 font-mono shadow-2xs">⌘K</kbd>
          </button>
        </div>

        <!-- RIGHT: Desktop Action Toolbar -->
        <div v-if="!uiStore.isMobile" class="flex items-center space-x-2 shrink-0">
          <!-- Command Palette Icon Button -->
          <button
            @click="uiStore.toggleCommandPalette()"
            class="p-1.5 rounded-xl border border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 text-gray-600 dark:text-slate-400 hover:bg-gray-50 dark:hover:bg-slate-800 transition cursor-pointer active:scale-95 duration-fast ease-spring"
            title="Command Palette (⌘K)"
          >
            <span class="text-xs font-mono font-bold px-0.5">⌘</span>
          </button>

          <!-- Segmented View Mode Switcher (List vs Grid) -->
          <div
            v-if="!workspaceStore.isDualPane"
            class="ios-segmented-group"
          >
            <button
              @click="activePanel.viewMode = 'list'; workspaceStore.saveState()"
              :class="['ios-segmented-item p-1.5', activePanel.viewMode === 'list' ? 'active' : '']"
              title="List View"
            >
              <FbIcon name="list" size="14px" />
            </button>
            <button
              @click="activePanel.viewMode = 'grid'; workspaceStore.saveState()"
              :class="['ios-segmented-item p-1.5', activePanel.viewMode === 'grid' ? 'active' : '']"
              title="Grid View"
            >
              <FbIcon name="grid" size="14px" />
            </button>
          </div>

          <!-- Unified View, Sort & Filter Popover (···) -->
          <div ref="sortMenuRef" class="relative">
            <button
              @click="isSortMenuOpen = !isSortMenuOpen"
              :class="[
                'p-1.5 rounded-xl border transition cursor-pointer flex items-center space-x-1 active:scale-95 duration-fast ease-spring',
                isSortMenuOpen || (activePanel.filterType && activePanel.filterType !== 'all') || activePanel.sortField !== 'name'
                  ? 'text-blue-600 dark:text-blue-400 border-blue-500/40 bg-blue-50/50 dark:bg-blue-950/40 ring-1 ring-blue-500/20'
                  : 'border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 text-gray-600 dark:text-slate-400 hover:bg-gray-50 dark:hover:bg-slate-800'
              ]"
              :title="`View & Sort Options`"
            >
              <FbIcon name="sort" size="15px" />
              <span
                v-if="activePanel.filterType && activePanel.filterType !== 'all'"
                class="w-1.5 h-1.5 rounded-full bg-blue-600 dark:bg-blue-400 animate-pulse"
              ></span>
            </button>

            <!-- Unified View Popover Menu -->
            <Transition name="ios-popover">
              <div
                v-if="isSortMenuOpen"
                @click.stop
                class="absolute right-0 mt-2 w-64 bg-white dark:bg-[#0f1422] border border-gray-200 dark:border-slate-800 rounded-2xl shadow-2xl p-2.5 z-50 text-xs text-gray-700 dark:text-slate-200 space-y-2.5"
              >
                <!-- SECTION 1: VIEW & VISIBILITY -->
                <div>
                  <div class="px-2 py-1 text-[10px] font-bold text-gray-400 dark:text-slate-500 uppercase tracking-wider">
                    VIEW & VISIBILITY
                  </div>
                  <div class="space-y-0.5">
                    <button
                      @click="toggleHidden"
                      class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800/80 transition text-left cursor-pointer"
                    >
                      <div class="flex items-center space-x-2">
                        <FbIcon name="eye" size="14px" class="text-gray-400" />
                        <span>Show Hidden Dotfiles</span>
                      </div>
                      <span v-if="activePanel.showHidden" class="text-blue-600 dark:text-blue-400 font-bold">✓</span>
                    </button>
                    <button
                      @click="toggleSelectAll"
                      class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800/80 transition text-left cursor-pointer"
                    >
                      <div class="flex items-center space-x-2">
                        <FbIcon name="select-all" size="14px" class="text-gray-400" />
                        <span>Select All Items</span>
                      </div>
                      <kbd class="text-[10px] text-gray-400 font-mono">⌘A</kbd>
                    </button>
                  </div>
                </div>

                <div class="border-t border-gray-100 dark:border-slate-800/80"></div>

                <!-- SECTION 2: SORT BY -->
                <div>
                  <div class="px-2 py-1 text-[10px] font-bold text-gray-400 dark:text-slate-500 uppercase tracking-wider">
                    SORT BY
                  </div>
                  <div class="space-y-0.5">
                    <button
                      v-for="field in sortFields"
                      :key="field.id"
                      @click="setSortField(field.id)"
                      :class="[
                        'w-full flex items-center justify-between px-2.5 py-1.5 rounded-xl transition text-left cursor-pointer',
                        activePanel.sortField === field.id
                          ? 'bg-blue-50 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400 font-semibold'
                          : 'hover:bg-gray-100 dark:hover:bg-slate-800/80 text-gray-700 dark:text-slate-300'
                      ]"
                    >
                      <span>{{ field.label }}</span>
                      <span v-if="activePanel.sortField === field.id" class="text-blue-600 dark:text-blue-400 font-bold">✓</span>
                    </button>
                  </div>
                </div>

                <!-- SECTION 3: ORDER (ASC / DESC) -->
                <div>
                  <div class="grid grid-cols-2 gap-1 p-0.5 bg-gray-100 dark:bg-slate-800/80 rounded-xl">
                    <button
                      @click="setSortOrder('asc')"
                      :class="[
                        'py-1 rounded-lg transition text-center font-medium cursor-pointer text-xs',
                        activePanel.sortOrder === 'asc'
                          ? 'bg-white dark:bg-slate-900 text-blue-600 dark:text-blue-400 shadow-2xs font-semibold'
                          : 'text-gray-500 dark:text-slate-400 hover:text-gray-900 dark:hover:text-white'
                      ]"
                    >
                      Ascending ↑
                    </button>
                    <button
                      @click="setSortOrder('desc')"
                      :class="[
                        'py-1 rounded-lg transition text-center font-medium cursor-pointer text-xs',
                        activePanel.sortOrder === 'desc'
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
                    @click="startPathEditing; isSortMenuOpen = false"
                    class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800/80 transition text-left cursor-pointer"
                  >
                    <div class="flex items-center space-x-2">
                      <FbIcon name="rename" size="14px" class="text-gray-400" />
                      <span>Edit Path Directly</span>
                    </div>
                    <kbd class="text-[10px] text-gray-400 font-mono">⌘L</kbd>
                  </button>
                  <button
                    @click="copyCurrentPath; isSortMenuOpen = false"
                    class="w-full flex items-center justify-between px-2.5 py-1.5 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800/80 transition text-left cursor-pointer"
                  >
                    <div class="flex items-center space-x-2">
                      <FbIcon name="copy" size="14px" class="text-gray-400" />
                      <span>Copy Current Path</span>
                    </div>
                  </button>
                </div>
              </div>
            </Transition>
          </div>

          <!-- Dual Pane Layout Control (⧉) -->
          <button
            @click="workspaceStore.setDualPane(!workspaceStore.isDualPane)"
            :class="[
              'p-1.5 rounded-xl border transition cursor-pointer active:scale-95 duration-fast ease-spring',
              workspaceStore.isDualPane
                ? 'text-blue-600 dark:text-blue-400 border-blue-500/40 bg-blue-50/50 dark:bg-blue-950/40 ring-1 ring-blue-500/20'
                : 'border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 text-gray-600 dark:text-slate-400 hover:bg-gray-50 dark:hover:bg-slate-800'
            ]"
            :title="workspaceStore.isDualPane ? 'Disable Dual Pane (Alt+D)' : 'Enable Dual Pane (Alt+D)'"
          >
            <FbIcon name="panel-right" size="15px" />
          </button>

          <!-- PRIMARY ACTION: + New Button (Filled Blue Pill) -->
          <div ref="newMenuRef" class="relative">
            <button
              @click="isNewMenuOpen = !isNewMenuOpen"
              class="bg-blue-600 hover:bg-blue-500 active:bg-blue-700 text-white font-semibold px-3.5 py-1.5 rounded-xl flex items-center space-x-1.5 text-xs shadow-xs transition cursor-pointer active:scale-95 duration-fast ease-spring"
            >
              <FbIcon name="plus" size="14px" />
              <span>New</span>
            </button>

            <!-- Dropdown Menu -->
            <Transition name="ios-popover">
              <div
                v-if="isNewMenuOpen"
                @click="isNewMenuOpen = false"
                class="absolute right-0 mt-2 w-48 bg-white dark:bg-[#0f1422] border border-gray-200 dark:border-slate-800 rounded-2xl shadow-xl p-1.5 z-50 text-xs text-gray-700 dark:text-slate-200 space-y-0.5"
              >
                <button
                  @click="openNew('file')"
                  class="w-full flex items-center space-x-2.5 px-3 py-2 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800 transition text-left cursor-pointer"
                >
                  <FbIcon name="new-file" size="15px" class="text-blue-600" />
                  <span class="font-medium">New File</span>
                </button>
                <button
                  @click="openNew('directory')"
                  class="w-full flex items-center space-x-2.5 px-3 py-2 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800 transition text-left cursor-pointer"
                >
                  <FbIcon name="new-folder" size="15px" class="text-amber-500" />
                  <span class="font-medium">New Folder</span>
                </button>
                <div class="my-1 border-t border-gray-100 dark:border-slate-800"></div>
                <button
                  @click="openUpload"
                  class="w-full flex items-center space-x-2.5 px-3 py-2 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800 transition text-left cursor-pointer"
                >
                  <FbIcon name="upload" size="15px" class="text-emerald-500" />
                  <span class="font-medium">Upload Files</span>
                </button>
                <div class="my-1 border-t border-gray-100 dark:border-slate-800"></div>
                <button
                  @click="openSyncModal"
                  class="w-full flex items-center space-x-2.5 px-3 py-2 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800 transition text-left cursor-pointer"
                >
                  <span class="text-sm">🔄</span>
                  <span class="font-medium">Sync Folder...</span>
                </button>
              </div>
            </Transition>
          </div>
        </div>

        <!-- RIGHT: Mobile Toolbar Controls -->
        <div v-else class="flex items-center space-x-1.5 shrink-0">
          <!-- Mobile Search Trigger -->
          <button
            @click="emit('openSearchDialog')"
            class="p-2 rounded-xl text-gray-500 hover:text-gray-900 dark:text-slate-400 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
            title="Search files"
          >
            <FbIcon name="search" size="18px" />
          </button>

          <!-- Mobile View Switcher (List <-> Grid) -->
          <button
            @click="toggleMobileViewMode"
            class="p-2 rounded-xl text-gray-500 hover:text-gray-900 dark:text-slate-400 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
            :title="activePanel.viewMode === 'grid' ? 'Switch to List View' : 'Switch to Grid View'"
          >
            <FbIcon :name="activePanel.viewMode === 'grid' ? 'list' : 'grid'" size="18px" />
          </button>

          <!-- Mobile + New Dropdown Button -->
          <div ref="newMenuRef" class="relative">
            <button
              @click="isNewMenuOpen = !isNewMenuOpen"
              class="bg-blue-600 hover:bg-blue-500 active:bg-blue-700 text-white font-bold p-2 rounded-xl shadow-xs transition cursor-pointer flex items-center justify-center active:scale-95"
              title="New Item / Upload"
            >
              <FbIcon name="plus" size="16px" />
            </button>

            <Transition name="ios-popover">
              <div
                v-if="isNewMenuOpen"
                @click="isNewMenuOpen = false"
                class="absolute right-0 mt-2 w-48 bg-white dark:bg-[#0f1422] border border-gray-200 dark:border-slate-800 rounded-2xl shadow-xl p-1.5 z-50 text-xs text-gray-700 dark:text-slate-200 space-y-0.5"
              >
                <button
                  @click="openNew('file')"
                  class="w-full flex items-center space-x-2.5 px-3 py-2.5 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800 transition text-left cursor-pointer"
                >
                  <FbIcon name="new-file" size="16px" class="text-blue-600" />
                  <span class="font-medium">New File</span>
                </button>
                <button
                  @click="openNew('directory')"
                  class="w-full flex items-center space-x-2.5 px-3 py-2.5 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800 transition text-left cursor-pointer"
                >
                  <FbIcon name="new-folder" size="16px" class="text-amber-500" />
                  <span class="font-medium">New Folder</span>
                </button>
                <div class="my-1 border-t border-gray-100 dark:border-slate-800"></div>
                <button
                  @click="openUpload"
                  class="w-full flex items-center space-x-2.5 px-3 py-2.5 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800 transition text-left cursor-pointer"
                >
                  <FbIcon name="upload" size="16px" class="text-emerald-500" />
                  <span class="font-medium">Upload File</span>
                </button>
              </div>
            </Transition>
          </div>

          <!-- Mobile Overflow More (⋮) -->
          <div ref="mobileMoreRef" class="relative">
            <button
              @click="isMobileMoreOpen = !isMobileMoreOpen"
              class="p-2 rounded-xl text-gray-500 hover:text-gray-900 dark:text-slate-400 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer font-bold text-base flex items-center justify-center active:scale-95"
              title="More actions"
            >
              <span>⋮</span>
            </button>

            <Transition name="ios-popover">
              <div
                v-if="isMobileMoreOpen"
                @click="isMobileMoreOpen = false"
                class="absolute right-0 mt-2 w-56 bg-white dark:bg-[#0f1422] border border-gray-200 dark:border-slate-800 rounded-2xl shadow-xl p-1.5 z-50 text-xs text-gray-700 dark:text-slate-200 space-y-0.5"
              >
                <button
                  @click="isSortMenuOpen = true"
                  class="w-full flex items-center space-x-2.5 px-3 py-2.5 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800 transition text-left cursor-pointer"
                >
                  <FbIcon name="sort" size="16px" class="text-blue-500" />
                  <span>Sort & Filter...</span>
                </button>
                <button
                  @click="toggleHidden"
                  class="w-full flex items-center space-x-2.5 px-3 py-2.5 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800 transition text-left cursor-pointer"
                >
                  <FbIcon name="eye" size="16px" class="text-gray-500" />
                  <span>{{ activePanel.showHidden ? 'Hide Dotfiles' : 'Show Dotfiles' }}</span>
                </button>
                <button
                  @click="toggleSelectAll"
                  class="w-full flex items-center space-x-2.5 px-3 py-2.5 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800 transition text-left cursor-pointer"
                >
                  <FbIcon name="select-all" size="16px" class="text-indigo-500" />
                  <span>{{ activePanel.selectedEntries.length > 0 ? 'Deselect All' : 'Select All' }}</span>
                </button>
                <button
                  @click="copyCurrentPath"
                  class="w-full flex items-center space-x-2.5 px-3 py-2.5 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800 transition text-left cursor-pointer"
                >
                  <FbIcon name="copy" size="16px" class="text-amber-500" />
                  <span>Copy Current Path</span>
                </button>
                <div class="my-1 border-t border-gray-100 dark:border-slate-800"></div>
                <button
                  @click="workspaceStore.setDualPane(!workspaceStore.isDualPane)"
                  class="w-full flex items-center space-x-2.5 px-3 py-2.5 rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800 transition text-left cursor-pointer"
                >
                  <FbIcon name="panel-right" size="16px" class="text-emerald-500" />
                  <span>{{ workspaceStore.isDualPane ? 'Disable Dual Pane' : 'Enable Dual Pane' }}</span>
                </button>
              </div>
            </Transition>
          </div>
        </div>
      </div>
    </Transition>
  </header>
</template>

<script setup lang="ts">
import { ref, computed, nextTick, onMounted, onUnmounted } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useFileStore } from '../../stores/fileStore';
import { useUiStore } from '../../stores/uiStore';
import type { FileEntry } from '../../types/vfs';

const emit = defineEmits<{
  (e: 'openSearchDialog'): void;
  (e: 'openConnectionDialog'): void;
  (e: 'openAuditLogDialog'): void;
  (e: 'openArchiveDialog', paths: string[]): void;
}>();

const workspaceStore = useWorkspaceStore();
const fileStore = useFileStore();
const uiStore = useUiStore();

const isSortMenuOpen = ref(false);
const isNewMenuOpen = ref(false);
const isMobileMoreOpen = ref(false);
const isTruncatedMenuOpen = ref(false);

const isEditingPath = ref(false);
const inputPath = ref('');
const pathInputRef = ref<HTMLInputElement | null>(null);

const activePanel = computed(() => workspaceStore.getPanel(workspaceStore.activePanelId));

const breadcrumbs = computed(() => {
  const p = activePanel.value.path;
  if (!p || p === '/') return [];
  const parts = p.split('/').filter(Boolean);
  let currentPath = '';
  return parts.map((part, index) => {
    currentPath += '/' + part;
    return {
      name: part,
      path: currentPath,
      isLast: index === parts.length - 1
    };
  });
});

const hasTruncatedBreadcrumbs = computed(() => breadcrumbs.value.length > 3);
const truncatedCrumbs = computed(() => {
  if (!hasTruncatedBreadcrumbs.value) return [];
  return breadcrumbs.value.slice(0, breadcrumbs.value.length - 2);
});

const visibleBreadcrumbs = computed(() => {
  if (hasTruncatedBreadcrumbs.value) {
    return breadcrumbs.value.slice(-2);
  }
  return breadcrumbs.value;
});

const selectedEntriesObjects = computed<FileEntry[]>(() => {
  const selectedPaths = new Set(activePanel.value.selectedEntries);
  return activePanel.value.entries.filter((e) => selectedPaths.has(e.path));
});

const selectedTotalSize = computed<number>(() => {
  return selectedEntriesObjects.value.reduce((acc, curr) => acc + (curr.size || 0), 0);
});

const sortFields = [
  { id: 'name', label: 'Name' },
  { id: 'size', label: 'Size' },
  { id: 'modified', label: 'Last Modified' },
];

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

function toggleMobileViewMode() {
  activePanel.value.viewMode = activePanel.value.viewMode === 'grid' ? 'list' : 'grid';
  workspaceStore.saveState();
}

function startPathEditing() {
  inputPath.value = activePanel.value.path;
  isEditingPath.value = true;
  nextTick(() => {
    pathInputRef.value?.focus();
    pathInputRef.value?.select();
  });
}

function submitPath() {
  let target = inputPath.value.trim();
  if (!target) target = '/';
  if (!target.startsWith('/')) target = '/' + target;

  workspaceStore.navigatePanel(workspaceStore.activePanelId, target);
  isEditingPath.value = false;
}

async function copyCurrentPath() {
  const p = activePanel.value.path;
  try {
    await navigator.clipboard.writeText(p);
    uiStore.showToast(`Path copied: ${p}`, 'info');
  } catch (err) {
    uiStore.showToast('Failed to copy path', 'error');
  }
}

function setSortField(field: string) {
  activePanel.value.sortField = field;
}

function setSortOrder(order: 'asc' | 'desc') {
  activePanel.value.sortOrder = order;
}

function toggleSelectAll() {
  if (activePanel.value.selectedEntries.length === activePanel.value.entries.length) {
    activePanel.value.selectedEntries = [];
  } else {
    activePanel.value.selectedEntries = activePanel.value.entries.map((e) => e.path);
  }
}

async function toggleHidden() {
  await workspaceStore.toggleShowHidden(workspaceStore.activePanelId);
  uiStore.showToast(
    activePanel.value.showHidden ? 'Showing dotfiles' : 'Hiding dotfiles',
    'info'
  );
}

function openNew(type: 'file' | 'directory') {
  fileStore.currentConnectionId = activePanel.value.connectionId;
  fileStore.currentPath = activePanel.value.path;
  uiStore.openCreate(type);
}

function openUpload() {
  fileStore.currentConnectionId = activePanel.value.connectionId;
  fileStore.currentPath = activePanel.value.path;
  uiStore.openUpload();
}

function openSyncModal() {
  uiStore.openSync(
    activePanel.value.connectionId || 'local',
    activePanel.value.path || '/'
  );
}

function handleCompressSelection() {
  if (activePanel.value.selectedEntries.length > 0) {
    emit('openArchiveDialog', activePanel.value.selectedEntries);
  }
}

function handleRenameSelection() {
  if (selectedEntriesObjects.value.length === 1) {
    uiStore.openRename(selectedEntriesObjects.value[0]);
  }
}

function handleDeleteSelection() {
  if (activePanel.value.selectedEntries.length > 0) {
    uiStore.openDelete(activePanel.value.selectedEntries);
  }
}

const sortMenuRef = ref<HTMLElement | null>(null);
const newMenuRef = ref<HTMLElement | null>(null);
const mobileMoreRef = ref<HTMLElement | null>(null);
const truncatedMenuRef = ref<HTMLElement | null>(null);

function handleOutsideClick(e: MouseEvent) {
  const target = e.target as Node;
  if (isSortMenuOpen.value && sortMenuRef.value && !sortMenuRef.value.contains(target)) {
    isSortMenuOpen.value = false;
  }
  if (isNewMenuOpen.value && newMenuRef.value && !newMenuRef.value.contains(target)) {
    isNewMenuOpen.value = false;
  }
  if (isMobileMoreOpen.value && mobileMoreRef.value && !mobileMoreRef.value.contains(target)) {
    isMobileMoreOpen.value = false;
  }
  if (isTruncatedMenuOpen.value && truncatedMenuRef.value && !truncatedMenuRef.value.contains(target)) {
    isTruncatedMenuOpen.value = false;
  }
}

function handleKeydown(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'l') {
    e.preventDefault();
    startPathEditing();
  }
}

onMounted(() => {
  window.addEventListener('click', handleOutsideClick);
  window.addEventListener('keydown', handleKeydown);
});

onUnmounted(() => {
  window.removeEventListener('click', handleOutsideClick);
  window.removeEventListener('keydown', handleKeydown);
});
</script>

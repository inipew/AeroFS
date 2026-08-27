<template>
  <header class="h-14 md:h-16 bg-white/95 dark:bg-[#0b0f19]/95 backdrop-blur-md border-b border-gray-200/80 dark:border-slate-800/80 px-3 sm:px-4 md:px-6 flex items-center justify-between text-gray-800 dark:text-slate-100 select-none sticky top-0 z-30 transition-colors duration-150">
    <!-- LEFT: Breadcrumbs, Storage Switcher & Path Navigator -->
    <div class="flex items-center space-x-1.5 max-w-[60vw] md:max-w-[55vw] text-sm md:text-base font-medium shrink min-w-0 py-1">
      <!-- Mobile Sidebar Drawer Toggle Button -->
      <button
        v-if="uiStore.isMobile"
        @click="uiStore.isMobileSidebarOpen = true"
        class="p-2 -ml-1 mr-0.5 text-gray-500 hover:text-gray-900 dark:text-slate-400 dark:hover:text-white rounded-xl hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer shrink-0"
        title="Open Navigation Drawer"
      >
        <FbIcon name="menu" size="19px" />
      </button>

      <!-- Parent Folder Navigation (Up / Back) -->
      <button
        v-if="activePanel.path !== '/'"
        @click="workspaceStore.navigateUp(workspaceStore.activePanelId)"
        :class="[
          'p-1.5 rounded-xl transition cursor-pointer shrink-0 font-bold flex items-center justify-center',
          uiStore.isMobile
            ? 'text-blue-600 dark:text-blue-400 hover:bg-blue-50 dark:hover:bg-blue-950/50'
            : 'text-gray-500 hover:text-gray-900 dark:text-slate-400 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-slate-800'
        ]"
        title="Go to parent directory (Alt+Up)"
      >
        <FbIcon name="chevron-left" size="18px" />
      </button>

      <!-- INLINE PATH EDITING BAR -->
      <div v-if="isEditingPath" class="flex items-center space-x-1 flex-1 min-w-0">
        <form @submit.prevent="submitPath" class="flex-1 flex items-center min-w-0">
          <input
            ref="pathInputRef"
            v-model="inputPath"
            type="text"
            placeholder="/path/to/folder"
            @keydown.esc="isEditingPath = false"
            class="w-full bg-white dark:bg-[#0f1422] border border-blue-500 text-gray-900 dark:text-white text-xs md:text-sm px-2.5 py-1 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500/30 font-mono shadow-xs"
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

      <!-- STANDARD BREADCRUMBS BAR -->
      <div v-else class="flex items-center space-x-0.5 shrink min-w-0 overflow-hidden">
        <!-- Root Storage Source Selector Dropdown -->
        <div ref="sourceMenuRef" class="relative shrink-0 flex items-center">
          <button
            @click="workspaceStore.navigatePanel(workspaceStore.activePanelId, '/')"
            :class="[
              'flex items-center space-x-1.5 px-2 py-1 rounded-lg transition truncate cursor-pointer text-xs md:text-sm',
              activePanel.path === '/'
                ? 'text-gray-950 dark:text-white font-bold bg-gray-100/70 dark:bg-slate-800/60'
                : 'text-gray-500 dark:text-slate-400 hover:text-gray-900 dark:hover:text-white hover:bg-gray-100/70 dark:hover:bg-slate-800/60'
            ]"
            :title="`Go to ${activeSourceName} (/)`"
          >
            <FbIcon
              :name="activeConnection?.provider === 'local' ? 'folder' : 'share'"
              size="15px"
              :class="activePanel.path === '/' ? 'text-blue-600 dark:text-blue-400' : 'text-gray-400 dark:text-slate-500'"
            />
            <span class="truncate max-w-[90px] sm:max-w-[150px] font-semibold">{{ activeSourceName }}</span>
          </button>

          <!-- Storage Dropdown Chevron Button -->
          <button
            @click.stop="isSourceMenuOpen = !isSourceMenuOpen"
            :class="[
              'p-1 text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 rounded-md transition cursor-pointer flex items-center justify-center',
              isSourceMenuOpen ? 'text-blue-600 dark:text-blue-400 bg-gray-100 dark:bg-slate-800' : ''
            ]"
            title="Switch Storage Source"
          >
            <FbIcon
              name="chevron-down"
              size="11px"
              :class="['transition-transform duration-200', isSourceMenuOpen ? 'rotate-180 text-blue-600 dark:text-blue-400' : '']"
            />
          </button>

          <!-- Storage Dropdown Popover Menu -->
          <div
            v-if="isSourceMenuOpen"
            @click="isSourceMenuOpen = false"
            class="absolute left-0 top-full mt-2 w-64 bg-white dark:bg-[#0f1422] border border-gray-200 dark:border-slate-800 rounded-2xl shadow-2xl p-1.5 z-50 text-xs text-gray-700 dark:text-slate-200 space-y-0.5 animate-in fade-in zoom-in-95 duration-100"
          >
            <div class="px-3 py-1.5 text-[10px] font-bold text-gray-400 dark:text-slate-500 uppercase tracking-wider">
              STORAGE SOURCES
            </div>
            <button
              v-for="conn in connStore.connections"
              :key="conn.id"
              @click="handleSelectSource(conn.id)"
              :class="[
                'w-full flex items-center justify-between px-3 py-2.5 rounded-xl transition text-left cursor-pointer group',
                activePanel.connectionId === conn.id
                  ? 'bg-blue-50 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400 font-semibold ring-1 ring-blue-500/20'
                  : 'hover:bg-gray-100 dark:hover:bg-slate-800/80 text-gray-700 dark:text-slate-300'
              ]"
            >
              <div class="flex items-center space-x-2.5 truncate">
                <FbIcon
                  :name="conn.provider === 'local' ? 'folder' : 'share'"
                  size="15px"
                  :class="activePanel.connectionId === conn.id ? 'text-blue-600 dark:text-blue-400' : 'text-gray-400 group-hover:text-gray-600 dark:group-hover:text-slate-200'"
                />
                <span class="truncate font-medium">{{ conn.name }}</span>
              </div>
              <div class="flex items-center space-x-1.5 shrink-0 ml-2">
                <span
                  v-if="conn.provider !== 'local'"
                  class="text-[9px] uppercase font-mono px-1.5 py-0.5 rounded bg-gray-200/70 dark:bg-slate-800 text-gray-500 dark:text-slate-400"
                >
                  {{ conn.provider }}
                </span>
                <span v-if="activePanel.connectionId === conn.id" class="text-blue-600 dark:text-blue-400 text-xs font-bold">✓</span>
              </div>
            </button>
            <div class="my-1 border-t border-gray-100 dark:border-slate-800/80"></div>
            <button
              @click="emit('openConnectionDialog')"
              class="w-full flex items-center space-x-2 px-3 py-2 rounded-xl hover:bg-blue-50 dark:hover:bg-blue-950/40 text-blue-600 dark:text-blue-400 font-semibold transition text-left cursor-pointer"
            >
              <FbIcon name="plus" size="14px" />
              <span>Add Storage Connection...</span>
            </button>
          </div>
        </div>

        <!-- Desktop Breadcrumbs Chain with Smart Truncation -->
        <template v-if="!uiStore.isMobile">
          <!-- Long Path Collapsed Intermediate Dropdown (...) -->
          <div v-if="hasTruncatedBreadcrumbs" ref="truncatedMenuRef" class="relative shrink-0 flex items-center">
            <span class="text-gray-400 dark:text-slate-600 px-0.5 shrink-0 text-xs">/</span>
            <button
              @click="isTruncatedMenuOpen = !isTruncatedMenuOpen"
              class="px-1.5 py-0.5 rounded-md text-gray-500 hover:text-gray-900 dark:text-slate-400 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer text-xs font-bold"
              title="Show intermediate folders"
            >
              ...
            </button>

            <!-- Truncated Paths Popover -->
            <div
              v-if="isTruncatedMenuOpen"
              @click="isTruncatedMenuOpen = false"
              class="absolute left-0 top-full mt-1.5 w-56 bg-white dark:bg-[#0f1422] border border-gray-200 dark:border-slate-800 rounded-xl shadow-xl p-1 z-50 text-xs space-y-0.5 animate-in fade-in duration-100"
            >
              <button
                v-for="crumb in truncatedCrumbs"
                :key="crumb.path"
                @click="workspaceStore.navigatePanel(workspaceStore.activePanelId, crumb.path)"
                class="w-full flex items-center space-x-2 px-2.5 py-1.5 rounded-lg hover:bg-gray-100 dark:hover:bg-slate-800 transition text-left text-gray-700 dark:text-slate-300"
              >
                <FbIcon name="folder" size="14px" class="text-amber-500 shrink-0" />
                <span class="truncate">{{ crumb.name }}</span>
              </button>
            </div>
          </div>

          <!-- Visible Tail Breadcrumbs -->
          <template v-for="crumb in visibleBreadcrumbs" :key="crumb.path">
            <span class="text-gray-400 dark:text-slate-600 px-0.5 shrink-0 text-xs">/</span>
            <button
              @click="workspaceStore.navigatePanel(workspaceStore.activePanelId, crumb.path)"
              :class="[
                'px-2 py-1 rounded-lg transition truncate shrink min-w-0 max-w-[120px] sm:max-w-[180px] text-xs md:text-sm',
                crumb.isLast
                  ? 'text-gray-950 dark:text-white font-bold bg-gray-100/50 dark:bg-slate-800/40'
                  : 'text-gray-500 dark:text-slate-400 hover:text-gray-900 dark:hover:text-white hover:bg-gray-100/70 dark:hover:bg-slate-800/60 cursor-pointer font-medium'
              ]"
              :title="crumb.name"
            >
              {{ crumb.name }}
            </button>
          </template>

          <!-- Edit Path & Copy Path Quick Actions (Desktop) -->
          <div class="flex items-center space-x-0.5 ml-1 opacity-0 hover:opacity-100 group-hover:opacity-100 transition-opacity">
            <button
              @click="startPathEditing"
              class="p-1 rounded-md text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
              title="Edit path directly (Ctrl+L)"
            >
              <FbIcon name="rename" size="12px" />
            </button>
            <button
              @click="copyCurrentPath"
              class="p-1 rounded-md text-gray-400 hover:text-gray-700 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
              title="Copy path to clipboard"
            >
              <FbIcon name="copy" size="12px" />
            </button>
          </div>
        </template>

        <!-- Mobile Clean Last Folder Segment with Tap to Edit -->
        <template v-else-if="breadcrumbs.length > 0">
          <span class="text-gray-400 dark:text-slate-600 px-0.5 shrink-0 text-xs">/</span>
          <button
            @click="startPathEditing"
            class="text-gray-950 dark:text-white font-bold truncate max-w-[120px] text-xs text-left px-1.5 py-0.5 rounded-lg active:bg-gray-100 dark:active:bg-slate-800 transition"
            title="Tap to edit path"
          >
            {{ breadcrumbs[breadcrumbs.length - 1].name }}
          </button>
        </template>
      </div>
    </div>

    <!-- RIGHT: MOBILE TOOLBAR (Search, List/Grid, + New, ⋮ More) -->
    <div v-if="uiStore.isMobile" class="flex items-center space-x-1 shrink-0">
      <!-- Search Button -->
      <button
        @click="emit('openSearchDialog')"
        class="p-2 rounded-xl text-gray-500 hover:text-gray-900 dark:text-slate-400 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
        title="Search files"
      >
        <FbIcon name="search" size="18px" />
      </button>

      <!-- View Switcher Toggle Button (List <-> Grid) -->
      <button
        @click="toggleMobileViewMode"
        class="p-2 rounded-xl text-gray-500 hover:text-gray-900 dark:text-slate-400 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
        :title="activePanel.viewMode === 'grid' ? 'Switch to List View' : 'Switch to Grid View'"
      >
        <FbIcon :name="activePanel.viewMode === 'grid' ? 'list' : 'grid'" size="18px" />
      </button>

      <!-- Mobile + New Dropdown -->
      <div ref="newMenuRef" class="relative">
        <button
          @click="isNewMenuOpen = !isNewMenuOpen"
          class="bg-blue-600 hover:bg-blue-700 active:bg-blue-800 text-white font-bold p-2 rounded-xl shadow-xs transition cursor-pointer flex items-center justify-center"
          title="New Item / Upload"
        >
          <FbIcon name="plus" size="17px" />
        </button>

        <div
          v-if="isNewMenuOpen"
          @click="isNewMenuOpen = false"
          class="absolute right-0 mt-2 w-48 bg-white dark:bg-[#0f1422] border border-gray-200 dark:border-slate-800 rounded-2xl shadow-xl p-1.5 z-50 text-xs text-gray-700 dark:text-slate-200 space-y-0.5 animate-in fade-in zoom-in-95 duration-100"
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
      </div>

      <!-- Mobile More Menu Button (⋮) -->
      <div ref="mobileMoreRef" class="relative">
        <button
          @click="isMobileMoreOpen = !isMobileMoreOpen"
          class="p-2 rounded-xl text-gray-500 hover:text-gray-900 dark:text-slate-400 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer font-bold text-base flex items-center justify-center"
          title="More actions"
        >
          <span>⋮</span>
        </button>

        <div
          v-if="isMobileMoreOpen"
          @click="isMobileMoreOpen = false"
          class="absolute right-0 mt-2 w-56 bg-white dark:bg-[#0f1422] border border-gray-200 dark:border-slate-800 rounded-2xl shadow-xl p-1.5 z-50 text-xs text-gray-700 dark:text-slate-200 space-y-0.5 animate-in fade-in zoom-in-95 duration-100"
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
      </div>
    </div>

    <!-- RIGHT: DESKTOP TOOLBAR CONTROLS -->
    <div v-else class="flex items-center space-x-2 shrink-0">
      <!-- Command Palette Search Trigger (Pill with ⌘K badge) -->
      <button
        @click="uiStore.toggleCommandPalette()"
        class="bg-gray-50/80 dark:bg-slate-900/80 hover:bg-gray-100/90 dark:hover:bg-slate-800/90 border border-gray-200/90 dark:border-slate-800 px-3.5 py-1.5 rounded-xl text-gray-400 hover:text-gray-600 dark:hover:text-slate-300 flex items-center space-x-2.5 text-xs transition shadow-2xs w-44 sm:w-52 justify-between cursor-pointer group"
        title="Command Palette (Ctrl+K / ⌘K)"
      >
        <div class="flex items-center space-x-2 truncate">
          <FbIcon name="search" size="14px" class="text-gray-400 group-hover:text-blue-500 transition-colors" />
          <span class="text-gray-400 font-medium truncate">Search or jump to...</span>
        </div>
        <kbd class="px-1.5 py-0.5 rounded bg-white dark:bg-slate-800 text-[10px] text-gray-500 dark:text-slate-400 border border-gray-200 dark:border-slate-700 font-mono shadow-2xs">⌘K</kbd>
      </button>

      <!-- View Switcher (List vs Grid) -->
      <div class="flex items-center border border-gray-200 dark:border-slate-800 rounded-xl p-0.5 bg-gray-50/70 dark:bg-slate-900">
        <!-- List View Button -->
        <button
          @click="activePanel.viewMode = 'list'; workspaceStore.saveState()"
          :class="[
            'p-1.5 rounded-lg transition cursor-pointer',
            activePanel.viewMode === 'list'
              ? 'bg-white dark:bg-slate-800 text-blue-600 dark:text-blue-400 shadow-2xs font-semibold'
              : 'text-gray-500 dark:text-slate-400 hover:text-gray-900 dark:hover:text-white'
          ]"
          title="List View"
        >
          <FbIcon name="list" size="15px" />
        </button>

        <!-- Grid View Button -->
        <button
          @click="activePanel.viewMode = 'grid'; workspaceStore.saveState()"
          :class="[
            'p-1.5 rounded-lg transition cursor-pointer',
            activePanel.viewMode === 'grid'
              ? 'bg-white dark:bg-slate-800 text-blue-600 dark:text-blue-400 shadow-2xs font-semibold'
              : 'text-gray-500 dark:text-slate-400 hover:text-gray-900 dark:hover:text-white'
          ]"
          title="Grid / Card View"
        >
          <FbIcon name="grid" size="15px" />
        </button>
      </div>

      <!-- Sort & Filter Dropdown Popover -->
      <div ref="sortMenuRef" class="relative">
        <button
          @click="isSortMenuOpen = !isSortMenuOpen"
          :class="[
            'p-1.5 rounded-xl border transition cursor-pointer flex items-center space-x-1',
            isSortMenuOpen || (activePanel.filterType && activePanel.filterType !== 'all') || activePanel.sortField !== 'name'
              ? 'text-blue-600 dark:text-blue-400 border-blue-500/40 bg-blue-50/50 dark:bg-blue-950/40 ring-1 ring-blue-500/20'
              : 'border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 text-gray-600 dark:text-slate-400 hover:bg-gray-50 dark:hover:bg-slate-800'
          ]"
          :title="`Sort & Filter (${activePanel.sortField}, ${activePanel.sortOrder})`"
        >
          <FbIcon name="sort" size="16px" />
          <span
            v-if="activePanel.filterType && activePanel.filterType !== 'all'"
            class="w-1.5 h-1.5 rounded-full bg-blue-600 dark:bg-blue-400 animate-pulse"
          ></span>
        </button>

        <!-- Sort & Filter Popover Menu -->
        <div
          v-if="isSortMenuOpen"
          @click.stop
          class="absolute right-0 mt-2 w-64 bg-white dark:bg-[#0f1422] border border-gray-200 dark:border-slate-800 rounded-2xl shadow-2xl p-2.5 z-50 text-xs text-gray-700 dark:text-slate-200 space-y-2.5 animate-in fade-in zoom-in-95 duration-100"
        >
          <!-- SECTION 1: SORT BY -->
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

          <div class="border-t border-gray-100 dark:border-slate-800/80"></div>

          <!-- SECTION 2: ORDER (ASC / DESC) -->
          <div>
            <div class="px-2 py-1 text-[10px] font-bold text-gray-400 dark:text-slate-500 uppercase tracking-wider">
              ORDER
            </div>
            <div class="grid grid-cols-2 gap-1 p-0.5 bg-gray-100 dark:bg-slate-800/80 rounded-xl">
              <button
                @click="setSortOrder('asc')"
                :class="[
                  'py-1 text-center rounded-lg font-medium transition cursor-pointer text-xs',
                  activePanel.sortOrder === 'asc'
                    ? 'bg-white dark:bg-slate-700 text-blue-600 dark:text-blue-400 shadow-2xs font-semibold'
                    : 'text-gray-500 hover:text-gray-900 dark:text-slate-400'
                ]"
              >
                Ascending
              </button>
              <button
                @click="setSortOrder('desc')"
                :class="[
                  'py-1 text-center rounded-lg font-medium transition cursor-pointer text-xs',
                  activePanel.sortOrder === 'desc'
                    ? 'bg-white dark:bg-slate-700 text-blue-600 dark:text-blue-400 shadow-2xs font-semibold'
                    : 'text-gray-500 hover:text-gray-900 dark:text-slate-400'
                ]"
              >
                Descending
              </button>
            </div>
          </div>

          <div class="border-t border-gray-100 dark:border-slate-800/80"></div>

          <!-- SECTION 3: FILTER BY TYPE -->
          <div>
            <div class="px-2 py-1 flex items-center justify-between text-[10px] font-bold text-gray-400 dark:text-slate-500 uppercase tracking-wider">
              <span>FILTER BY TYPE</span>
              <button
                v-if="activePanel.filterType && activePanel.filterType !== 'all'"
                @click="setFilterType('all')"
                class="text-blue-600 dark:text-blue-400 hover:underline cursor-pointer font-medium normal-case text-[10px]"
              >
                Reset
              </button>
            </div>
            <div class="space-y-0.5 max-h-40 overflow-y-auto">
              <button
                v-for="flt in filterOptions"
                :key="flt.id"
                @click="setFilterType(flt.id)"
                :class="[
                  'w-full flex items-center justify-between px-2.5 py-1.5 rounded-xl transition text-left cursor-pointer',
                  (activePanel.filterType || 'all') === flt.id
                    ? 'bg-blue-50 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400 font-semibold'
                    : 'hover:bg-gray-100 dark:hover:bg-slate-800/80 text-gray-700 dark:text-slate-300'
                ]"
              >
                <div class="flex items-center space-x-2">
                  <span class="text-sm">{{ flt.icon }}</span>
                  <span>{{ flt.label }}</span>
                </div>
                <span v-if="(activePanel.filterType || 'all') === flt.id" class="text-blue-600 dark:text-blue-400 font-bold">✓</span>
              </button>
            </div>
          </div>
        </div>
      </div>

      <!-- Select Mode / Toggle All Button -->
      <button
        @click="toggleSelectAll"
        :class="[
          'p-1.5 rounded-xl border transition cursor-pointer',
          activePanel.selectedEntries.length > 0
            ? 'text-blue-600 dark:text-blue-400 border-blue-500/40 bg-blue-50/50 dark:bg-blue-950/40 ring-1 ring-blue-500/20'
            : 'border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 text-gray-600 dark:text-slate-400 hover:bg-gray-50 dark:hover:bg-slate-800'
        ]"
        :title="activePanel.selectedEntries.length > 0 ? `Deselect all (${activePanel.selectedEntries.length} selected)` : 'Select all items (Ctrl+A)'"
      >
        <FbIcon name="select-all" size="16px" />
      </button>

      <!-- Show/Hide Hidden Files Toggle Button -->
      <button
        @click="toggleHidden"
        :class="[
          'p-1.5 rounded-xl border transition cursor-pointer',
          activePanel.showHidden
            ? 'text-blue-600 dark:text-blue-400 border-blue-500/40 bg-blue-50/50 dark:bg-blue-950/40 ring-1 ring-blue-500/20'
            : 'border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 text-gray-600 dark:text-slate-400 hover:bg-gray-50 dark:hover:bg-slate-800'
        ]"
        :title="activePanel.showHidden ? 'Hide Dotfiles (Ctrl+H)' : 'Show Dotfiles (Ctrl+H)'"
      >
        <FbIcon name="eye" size="16px" />
      </button>

      <!-- Dual Pane Switcher Button -->
      <button
        @click="workspaceStore.setDualPane(!workspaceStore.isDualPane)"
        :class="[
          'p-1.5 rounded-xl border transition cursor-pointer',
          workspaceStore.isDualPane
            ? 'text-blue-600 dark:text-blue-400 border-blue-500/40 bg-blue-50/50 dark:bg-blue-950/40 ring-1 ring-blue-500/20'
            : 'border-gray-200 dark:border-slate-800 bg-white dark:bg-slate-900 text-gray-600 dark:text-slate-400 hover:bg-gray-50 dark:hover:bg-slate-800'
        ]"
        :title="workspaceStore.isDualPane ? 'Disable Dual Pane (Alt+D)' : 'Enable Dual Pane (Alt+D)'"
      >
        <FbIcon name="panel-right" size="16px" />
      </button>

      <!-- Primary Action Button: + New Dropdown -->
      <div ref="newMenuRef" class="relative">
        <button
          @click="isNewMenuOpen = !isNewMenuOpen"
          class="bg-blue-600 hover:bg-blue-700 active:bg-blue-800 text-white font-semibold px-3.5 py-1.5 rounded-xl flex items-center space-x-1.5 text-xs shadow-xs transition cursor-pointer"
        >
          <FbIcon name="plus" size="14px" />
          <span>New</span>
        </button>

        <!-- Dropdown Menu -->
        <div
          v-if="isNewMenuOpen"
          @click="isNewMenuOpen = false"
          class="absolute right-0 mt-2 w-48 bg-white dark:bg-[#0f1422] border border-gray-200 dark:border-slate-800 rounded-2xl shadow-xl p-1.5 z-50 text-xs text-gray-700 dark:text-slate-200 space-y-0.5 animate-in fade-in zoom-in-95 duration-100"
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
        </div>
      </div>
    </div>
  </header>
</template>

<script setup lang="ts">
import { ref, computed, nextTick, onMounted, onUnmounted } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useConnectionStore } from '../../stores/connectionStore';
import { useFileStore } from '../../stores/fileStore';
import { useUiStore } from '../../stores/uiStore';

const emit = defineEmits<{
  (e: 'openSearchDialog'): void;
  (e: 'openConnectionDialog'): void;
  (e: 'openAuditLogDialog'): void;
}>();

const workspaceStore = useWorkspaceStore();
const connStore = useConnectionStore();
const fileStore = useFileStore();
const uiStore = useUiStore();

const isSourceMenuOpen = ref(false);
const isSortMenuOpen = ref(false);
const isNewMenuOpen = ref(false);
const isMobileMoreOpen = ref(false);
const isTruncatedMenuOpen = ref(false);

// Direct Path Editor State
const isEditingPath = ref(false);
const inputPath = ref('');
const pathInputRef = ref<HTMLInputElement | null>(null);

const activePanel = computed(() => workspaceStore.getPanel(workspaceStore.activePanelId));

const activeConnection = computed(() => {
  return connStore.connections.find((c) => c.id === activePanel.value.connectionId);
});

const activeSourceName = computed(() => {
  if (activeConnection.value) {
    return activeConnection.value.name;
  }
  return activePanel.value.connectionId === 'local' ? 'Local Storage' : activePanel.value.connectionId;
});

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

// Smart Breadcrumb Truncation: if depth > 3, collapse middle items into "..."
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

const sortFields = [
  { id: 'name', label: 'Name' },
  { id: 'size', label: 'Size' },
  { id: 'modified', label: 'Last Modified' },
];

const filterOptions = [
  { id: 'all', label: 'All Files', icon: '📁' },
  { id: 'documents', label: 'Documents', icon: '📄' },
  { id: 'images', label: 'Images & Photos', icon: '🖼️' },
  { id: 'videos', label: 'Videos & Movies', icon: '🎥' },
  { id: 'audio', label: 'Audio & Music', icon: '🎵' },
  { id: 'archives', label: 'Archives (Zip/Tar)', icon: '📦' },
  { id: 'code', label: 'Code & Scripts', icon: '💻' },
];

function handleSelectSource(connectionId: string) {
  isSourceMenuOpen.value = false;
  workspaceStore.switchPanelConnection(workspaceStore.activePanelId, connectionId, '/');
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

function setFilterType(type: string) {
  activePanel.value.filterType = type;
  isSortMenuOpen.value = false;
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
    activePanel.value.showHidden ? 'Showing dotfiles & hidden items' : 'Hiding dotfiles',
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

const sourceMenuRef = ref<HTMLElement | null>(null);
const sortMenuRef = ref<HTMLElement | null>(null);
const newMenuRef = ref<HTMLElement | null>(null);
const mobileMoreRef = ref<HTMLElement | null>(null);
const truncatedMenuRef = ref<HTMLElement | null>(null);

function handleOutsideClick(e: MouseEvent) {
  const target = e.target as Node;
  if (isSourceMenuOpen.value && sourceMenuRef.value && !sourceMenuRef.value.contains(target)) {
    isSourceMenuOpen.value = false;
  }
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
  // Ctrl+L to edit path
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


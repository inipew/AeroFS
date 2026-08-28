<template>
  <div
    @click="workspaceStore.setActivePanel(panelId)"
    :class="[
      'flex-1 flex flex-col h-full bg-white dark:bg-[#0b0f19] overflow-hidden relative select-none',
      workspaceStore.isDualPane
        ? (isActive
            ? 'border-t-2 border-t-blue-600 dark:border-t-blue-500'
            : 'border-t-2 border-t-transparent opacity-90')
        : ''
    ]"
  >
    <!-- Dual-Pane Subheader with Navigation Engine, Connection Switcher & Actions -->
    <div
      v-if="workspaceStore.isDualPane"
      :class="[
        'h-11 sm:h-12 border-b px-3 sm:px-4 flex items-center justify-between transition-colors text-xs shrink-0 backdrop-blur-md',
        isActive
          ? 'bg-blue-50/40 dark:bg-[#0d1424]/90 border-gray-200 dark:border-slate-800 text-gray-900 dark:text-white font-medium'
          : 'bg-gray-50/60 dark:bg-[#080c16]/90 border-gray-200/80 dark:border-slate-800/80 text-gray-500 dark:text-slate-400'
      ]"
    >
      <div class="flex items-center space-x-1.5 truncate">
        <!-- Navigation Buttons: Back, Forward, Up -->
        <div class="flex items-center bg-gray-100/80 dark:bg-slate-800/60 rounded-xl p-0.5 border border-gray-200/70 dark:border-slate-700/60">
          <button
            @click.stop="workspaceStore.goBack(panelId)"
            :disabled="panel.historyIndex <= 0"
            class="p-1 rounded-lg text-gray-500 hover:text-gray-900 dark:text-slate-400 dark:hover:text-slate-100 hover:bg-white dark:hover:bg-slate-700 transition cursor-pointer disabled:opacity-30 disabled:cursor-not-allowed"
            title="Back (Alt+Left)"
          >
            <FbIcon name="chevron-left" size="13px" />
          </button>

          <button
            @click.stop="workspaceStore.goForward(panelId)"
            :disabled="panel.historyIndex >= panel.history.length - 1"
            class="p-1 rounded-lg text-gray-500 hover:text-gray-900 dark:text-slate-400 dark:hover:text-slate-100 hover:bg-white dark:hover:bg-slate-700 transition cursor-pointer disabled:opacity-30 disabled:cursor-not-allowed"
            title="Forward (Alt+Right)"
          >
            <FbIcon name="chevron-right" size="13px" />
          </button>

          <button
            @click.stop="workspaceStore.navigateUp(panelId)"
            :disabled="panel.path === '/' || panel.path === ''"
            class="p-1 rounded-lg text-gray-500 hover:text-gray-900 dark:text-slate-400 dark:hover:text-slate-100 hover:bg-white dark:hover:bg-slate-700 transition cursor-pointer disabled:opacity-30 disabled:cursor-not-allowed"
            title="Up (Alt+Up)"
          >
            <FbIcon name="arrow-up" size="13px" />
          </button>
        </div>

        <div class="h-3.5 w-px bg-gray-200 dark:bg-slate-800 mx-1 shrink-0"></div>

        <!-- Address Bar Mode vs Interactive Breadcrumb Path -->
        <div v-if="isAddressBar" class="relative flex items-center flex-1 min-w-0">
          <input
            ref="addressInputRef"
            v-model="addressInput"
            @keydown.enter="submitAddressBar"
            @keydown.esc="isAddressBar = false"
            @keydown.tab.prevent="autocompleteFirstPath"
            @blur="handleAddressBlur"
            type="text"
            class="w-full bg-white dark:bg-[#0f1422] border border-blue-500 rounded-xl px-2.5 py-1 text-xs font-mono text-gray-800 dark:text-slate-100 outline-none shadow-xs"
            placeholder="/path/to/folder"
          />
          <!-- Autocomplete Dropdown -->
          <div
            v-if="pathSuggestions.length > 0"
            class="absolute top-full left-0 mt-1 w-full bg-white dark:bg-slate-900 border border-gray-200 dark:border-slate-800 rounded-xl shadow-xl z-50 py-1 max-h-40 overflow-y-auto text-[11px] font-mono"
          >
            <div
              v-for="sug in pathSuggestions"
              :key="sug"
              @mousedown.prevent="applySuggestion(sug)"
              class="px-2.5 py-1 hover:bg-blue-50 dark:hover:bg-blue-950/40 text-gray-700 dark:text-slate-300 hover:text-blue-600 dark:hover:text-blue-400 cursor-pointer flex items-center space-x-1.5 truncate"
            >
              <span class="text-xs">📁</span>
              <span class="truncate">{{ sug }}</span>
            </div>
          </div>
        </div>

        <!-- Interactive Clickable Segmented Breadcrumb Path -->
        <div
          v-else
          class="flex items-center min-w-0 flex-1 overflow-x-auto no-scrollbar py-0.5 space-x-1"
        >
          <!-- Root Button (/) -->
          <button
            @click.stop="workspaceStore.navigateTo(panelId, '/')"
            :class="[
              'px-2 py-0.5 rounded-lg hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer flex items-center space-x-1.5 shrink-0 border',
              panel.path === '/'
                ? 'bg-blue-50 dark:bg-blue-950/40 text-blue-600 dark:text-blue-400 border-blue-200 dark:border-blue-800/60 font-bold'
                : 'bg-transparent text-gray-600 dark:text-slate-400 border-transparent hover:border-gray-200 dark:hover:border-slate-700 font-medium'
            ]"
            title="Root (/)"
          >
            <FbIcon :name="panel.connectionId === 'local' ? 'folder' : 'share'" size="13px" class="text-blue-500 shrink-0" />
            <span class="text-xs font-semibold">{{ currentConnName }}</span>
          </button>

          <!-- Breadcrumb Segments -->
          <template v-for="(seg, idx) in breadcrumbSegments" :key="seg.path">
            <span class="text-gray-300 dark:text-slate-600 text-xs shrink-0 select-none">/</span>
            <button
              @click.stop="workspaceStore.navigateTo(panelId, seg.path)"
              :class="[
                'px-1.5 py-0.5 rounded-lg hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer text-xs truncate max-w-[120px] sm:max-w-[160px]',
                idx === breadcrumbSegments.length - 1
                  ? 'font-bold text-gray-900 dark:text-white bg-gray-100/70 dark:bg-slate-800/60'
                  : 'text-gray-500 dark:text-slate-400 hover:text-gray-800 dark:hover:text-slate-200'
              ]"
              :title="seg.path"
            >
              {{ seg.name }}
            </button>
          </template>

          <!-- Edit Path Pencil / Trigger Button -->
          <button
            @click.stop="openAddressBar"
            class="p-1 rounded-lg text-gray-400 hover:text-gray-700 dark:text-slate-500 dark:hover:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer shrink-0 ml-0.5"
            title="Edit path directly (Ctrl+L)"
          >
            <FbIcon name="rename" size="11px" />
          </button>
        </div>
      </div>

      <!-- Panel Action Buttons: View Switcher, Reload, Swap & Close -->
      <div class="flex items-center space-x-1 shrink-0">
        <!-- Grid / List Toggle -->
        <button
          @click.stop="panel.viewMode = panel.viewMode === 'grid' ? 'list' : 'grid'"
          class="p-1.5 rounded-lg text-gray-500 hover:text-gray-900 dark:text-slate-400 dark:hover:text-slate-100 hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer"
          :title="panel.viewMode === 'grid' ? 'Switch to List View' : 'Switch to Grid View'"
        >
          <FbIcon :name="panel.viewMode === 'grid' ? 'list' : 'grid'" size="13px" />
        </button>

        <button
          :disabled="panel.loading"
          @click.stop="workspaceStore.refreshPanel(panelId)"
          class="p-1.5 rounded-lg text-gray-500 hover:text-gray-900 dark:text-slate-400 dark:hover:text-slate-100 hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer disabled:opacity-50"
          title="Reload panel (F5)"
        >
          <FbIcon name="refresh" size="13px" :class="{ 'animate-spin': panel.loading }" />
        </button>

        <button
          @click.stop="workspaceStore.swapPanels()"
          class="p-1.5 rounded-lg text-gray-500 hover:text-gray-900 dark:text-slate-400 dark:hover:text-slate-100 hover:bg-gray-100 dark:hover:bg-slate-800 transition cursor-pointer text-xs font-semibold"
          title="Swap Left & Right Panels (Alt+S)"
        >
          ⇄
        </button>

        <button
          @click.stop="workspaceStore.closePanel(panelId)"
          class="p-1.5 rounded-lg text-gray-400 hover:text-red-600 hover:bg-red-50 dark:hover:bg-red-950/40 transition cursor-pointer font-bold text-xs"
          title="Close panel"
        >
          ✕
        </button>
      </div>
    </div>

    <!-- Pinned Drop Overlay (Clean subtle dashed surface) -->
    <div
      v-if="isDragOver"
      :class="[
        'absolute inset-x-4 bottom-4 z-30 bg-blue-500/10 backdrop-blur-xs border-2 border-dashed border-blue-500 rounded-3xl flex items-center justify-center pointer-events-none transition-all duration-150',
        workspaceStore.isDualPane ? 'top-14' : 'top-4'
      ]"
    >
      <div class="bg-blue-600 text-white px-5 py-2.5 rounded-2xl shadow-xl font-bold text-xs flex items-center space-x-2">
        <FbIcon name="upload" size="18px" />
        <span>Drop files to copy into this folder</span>
      </div>
    </div>

    <!-- Graceful Warning / Stale Cache Banner (Human-Friendly Diagnostic) -->
    <div
      v-if="panel.error && panel.entries.length > 0"
      class="mx-4 mt-2 px-3.5 py-2 rounded-2xl bg-amber-500/10 border border-amber-500/30 text-amber-800 dark:text-amber-300 flex items-center justify-between text-xs shrink-0 animate-in fade-in"
    >
      <div class="flex items-center space-x-2.5 truncate mr-2">
        <FbIcon name="info" size="16px" class="text-amber-500 shrink-0" />
        <div class="truncate">
          <span class="font-bold">Unable to refresh directory</span>
          <span class="ml-1 text-[11px] opacity-80 truncate">Showing last cached version</span>
        </div>
      </div>
      <button
        @click.stop="workspaceStore.refreshPanel(panelId)"
        class="px-2.5 py-1 bg-amber-500 hover:bg-amber-600 text-white font-bold rounded-xl text-[10px] shrink-0 cursor-pointer shadow-xs transition"
      >
        Retry
      </button>
    </div>

    <!-- Drop Zone & File Listing Content (with Touch Pull-To-Refresh on Mobile) -->
    <div
      ref="panelContentRef"
      @click="handleContainerClick($event)"
      @dragenter="handleDragEnter"
      @dragover="handleDragOver"
      @dragleave="handleDragLeave"
      @drop.prevent="handleDrop($event)"
      @contextmenu.self="openBlankContextMenu"
      @touchstart.passive="onContainerTouchStart"
      @touchmove="onContainerTouchMove"
      @touchend="onContainerTouchEnd"
      @touchcancel="onContainerTouchEnd"
      class="flex-1 overflow-y-auto p-4 sm:p-5 relative"
    >
      <!-- Pull-to-Refresh Indicator Banner -->
      <div
        v-if="pullDistance > 0 || isPullRefreshing"
        :style="{ height: `${Math.min(50, pullDistance)}px` }"
        class="flex items-center justify-center overflow-hidden transition-all text-xs font-semibold text-blue-600 dark:text-blue-400 bg-blue-50/50 dark:bg-blue-950/20 rounded-xl mb-3"
      >
        <div class="flex items-center space-x-2">
          <span :class="['transition-transform duration-200 text-sm font-bold', pullDistance >= 40 || isPullRefreshing ? 'rotate-180 animate-spin' : '']">⟳</span>
          <span>{{ isPullRefreshing ? 'Refreshing...' : (pullDistance >= 40 ? 'Release to refresh' : 'Pull down to refresh') }}</span>
        </div>
      </div>

      <!-- Empty State -->
      <div
        v-if="displayedFolders.length === 0 && displayedFiles.length === 0 && !panel.loading"
        class="py-24 flex flex-col items-center justify-center text-center"
      >
        <div class="w-16 h-16 rounded-2xl bg-gray-100 dark:bg-slate-800 flex items-center justify-center text-gray-400 mb-3">
          <FbIcon name="empty-folder" size="32px" />
        </div>
        <p class="font-semibold text-gray-800 dark:text-slate-200 text-base">This folder is empty</p>
        <p class="text-xs text-gray-400 dark:text-slate-500 mt-1">Upload files or create new folders to get started</p>
      </div>

      <!-- MOSAIC / GRID VIEW (Explorer / Dolphin / Filestash Aesthetic - Responsively Auto-Filling) -->
      <div v-if="panel.viewMode === 'grid'" class="space-y-6">
        <!-- 1. FOLDERS SECTION -->
        <div v-if="displayedFolders.length > 0 || (panel.path !== '/' && panel.path !== '')">
          <div class="flex items-center justify-between mb-3 px-0.5">
            <div class="flex items-center space-x-2">
              <span class="text-[11px] font-bold uppercase tracking-wider text-gray-400 dark:text-slate-500">Folders</span>
              <span class="px-2 py-0.5 rounded-full bg-blue-50 dark:bg-blue-950/40 text-blue-600 dark:text-blue-400 font-mono font-bold text-[10px]">
                {{ displayedFolders.length }}
              </span>
            </div>
          </div>

          <div class="grid gap-3 sm:gap-3.5" style="grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));">
            <!-- Parent Folder Navigation Card (..) -->
            <div
              v-if="panel.path !== '/' && panel.path !== ''"
              @click="workspaceStore.navigateUp(panelId)"
              @dblclick="workspaceStore.navigateUp(panelId)"
              class="border border-dashed border-gray-300/80 dark:border-slate-700/80 hover:border-blue-500 dark:hover:border-blue-400 rounded-2xl p-3 flex flex-col items-center justify-between text-center cursor-pointer transition-all duration-200 select-none shadow-xs group bg-gray-50/60 dark:bg-slate-900/40 hover:bg-blue-50/40 dark:hover:bg-blue-950/30 hover:-translate-y-0.5 active:scale-[0.98] min-h-[124px] sm:min-h-[132px]"
              title="Go to parent directory (..)"
            >
              <div class="flex-1 flex items-center justify-center w-full py-1">
                <div class="w-11 h-11 rounded-2xl bg-gradient-to-tr from-blue-500/10 to-indigo-500/10 dark:from-blue-500/20 dark:to-indigo-500/20 text-blue-600 dark:text-blue-400 flex items-center justify-center group-hover:scale-110 transition-transform duration-200 ring-1 ring-blue-500/20">
                  <FbIcon name="arrow-up" size="18px" class="group-hover:-translate-y-0.5 transition-transform" />
                </div>
              </div>
              <span class="font-bold text-xs truncate text-gray-700 dark:text-slate-300 group-hover:text-blue-600 dark:group-hover:text-blue-400 w-full block">.. Parent</span>
            </div>

            <!-- Modern Azure Folder Card matching Filestash / Dolphin -->
            <div
              v-for="folder in displayedFolders"
              :key="folder.path"
              data-entry-item="true"
              :data-entry-path="folder.path"
              draggable="true"
              @dragstart="handleDragStart($event, folder)"
              @touchstart.passive="handleTouchStart($event, folder)"
              @touchend="handleTouchEnd"
              @touchmove="handleTouchMove"
              @touchcancel="handleTouchEnd"
              @click="handleEntryClick($event, folder)"
              @dblclick="handleEntryDoubleClick(folder)"
              @contextmenu="openContextMenu($event, folder)"
              @dragover.stop.prevent="handleFolderDragOver($event, folder)"
              @dragleave.stop="handleFolderDragLeave(folder)"
              @drop.stop.prevent="handleDrop($event, folder)"
              :class="[
                'border rounded-2xl p-3 flex flex-col items-center justify-between text-center cursor-pointer transition-all duration-200 select-none shadow-xs group active:scale-[0.98] min-h-[124px] sm:min-h-[132px]',
                isItemHidden(folder) ? 'opacity-65 hover:opacity-100 border-dashed border-gray-300 dark:border-slate-700 bg-gray-50/50 dark:bg-slate-900/40' : '',
                workspaceStore.isCutItem(panel.connectionId, folder.path) ? 'opacity-40 border-dashed border-amber-500 ring-1 ring-amber-500/30' : '',
                hoveredFolderDrop === folder.path
                  ? 'ring-2 ring-blue-500 scale-[1.04] bg-blue-100/70 dark:bg-blue-900/60 border-blue-500 shadow-lg'
                  : (panel.selectedEntries.includes(folder.path)
                    ? 'bg-blue-50/80 dark:bg-blue-950/50 border-blue-500 ring-2 ring-blue-500/30 shadow-md'
                    : 'bg-white dark:bg-[#0f1422] border-gray-200/90 dark:border-slate-800/90 hover:border-blue-400 dark:hover:border-blue-500 hover:shadow-lg hover:shadow-blue-500/5 hover:-translate-y-1')
              ]"
            >
              <!-- Large Prominent Sky-Blue / Azure Folder Icon -->
              <div class="flex-1 flex items-center justify-center w-full py-1">
                <svg viewBox="0 0 64 64" fill="none" xmlns="http://www.w3.org/2000/svg" class="w-12 h-12 sm:w-13 sm:h-13 drop-shadow-xs group-hover:scale-110 transition-transform duration-200">
                  <path d="M6 18C6 14.6863 8.68629 12 12 12H24.3431C25.9345 12 27.4609 12.6321 28.5858 13.7574L32.4142 17.5858C33.5391 18.7107 35.0655 19.3431 36.6569 19.3431H52C55.3137 19.3431 58 22.0294 58 25.3431V46C58 49.3137 55.3137 52 52 52H12C8.68629 52 6 49.3137 6 46V18Z" class="fill-sky-500 dark:fill-sky-600" />
                  <path d="M6 25C6 21.6863 8.68629 19 12 19H52C55.3137 19 58 21.6863 58 25V46C58 49.3137 55.3137 52 52 52H12C8.68629 52 6 49.3137 6 46V25Z" class="fill-sky-400 dark:fill-sky-400" />
                </svg>
              </div>

              <!-- Centered Folder Name Label (2-line wrap) -->
              <div class="w-full px-0.5 mt-1 text-center">
                <span class="font-semibold text-xs text-gray-800 dark:text-slate-100 group-hover:text-blue-600 dark:group-hover:text-blue-400 transition line-clamp-2 break-all leading-tight block" :title="folder.name">
                  {{ folder.name }}
                </span>
                <span v-if="isItemHidden(folder)" class="inline-block mt-0.5 text-[8px] px-1 py-0.2 rounded bg-gray-200/80 dark:bg-slate-800 text-gray-400 dark:text-slate-500 font-mono">
                  dot
                </span>
              </div>
            </div>
          </div>
        </div>

        <!-- 2. FILES SECTION -->
        <div v-if="displayedFiles.length > 0">
          <div class="flex items-center justify-between mb-3 px-0.5">
            <div class="flex items-center space-x-2">
              <span class="text-[11px] font-bold uppercase tracking-wider text-gray-400 dark:text-slate-500">Files</span>
              <span class="px-2 py-0.5 rounded-full bg-slate-100 dark:bg-slate-800 text-gray-600 dark:text-slate-300 font-mono font-bold text-[10px]">
                {{ displayedFiles.length }}
              </span>
            </div>
          </div>

          <div class="grid gap-3 sm:gap-3.5" style="grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));">
            <div
              v-for="file in displayedFiles"
              :key="file.path"
              data-entry-item="true"
              :data-entry-path="file.path"
              draggable="true"
              @dragstart="handleDragStart($event, file)"
              @touchstart.passive="handleTouchStart($event, file)"
              @touchend="handleTouchEnd"
              @touchmove="handleTouchMove"
              @touchcancel="handleTouchEnd"
              @click="handleEntryClick($event, file)"
              @dblclick="handleEntryDoubleClick(file)"
              @contextmenu="openContextMenu($event, file)"
              :class="[
                'border rounded-2xl overflow-hidden cursor-pointer transition-all duration-200 flex flex-col group select-none shadow-xs active:scale-[0.98] min-h-[136px] sm:min-h-[148px]',
                isItemHidden(file) ? 'opacity-65 hover:opacity-100 border-dashed border-gray-300 dark:border-slate-700 bg-gray-50/30 dark:bg-slate-900/30' : '',
                workspaceStore.isCutItem(panel.connectionId, file.path) ? 'opacity-40 border-dashed border-amber-500 ring-1 ring-amber-500/30' : '',
                panel.selectedEntries.includes(file.path)
                  ? 'bg-blue-50/80 dark:bg-blue-950/50 border-blue-500 ring-2 ring-blue-500/30 shadow-md'
                  : 'bg-white dark:bg-[#0f1422] border-gray-200/90 dark:border-slate-800/90 hover:shadow-lg hover:shadow-blue-500/5 hover:-translate-y-1 hover:border-blue-400 dark:hover:border-blue-500'
              ]"
            >
              <!-- Card Thumbnail Area (Centered Absolute Overlays) -->
              <div
                class="flex-1 w-full bg-slate-50/80 dark:bg-slate-950/70 relative overflow-hidden shrink-0 border-b border-gray-100 dark:border-slate-800/80 flex items-center justify-center p-2 min-h-[85px]"
              >
                <!-- Real Image Preview -->
                <template v-if="isImage(file)">
                  <img
                    :src="getDownloadUrl(panel.connectionId, file.path)"
                    :alt="file.name"
                    class="w-full h-full object-cover group-hover:scale-105 transition duration-300 rounded-lg"
                    loading="lazy"
                  />
                  <span class="absolute bottom-1.5 right-1.5 text-[8px] px-1 py-0.2 rounded-md bg-black/75 backdrop-blur-xs text-white/90 font-mono font-bold uppercase tracking-wider shadow-md border border-white/10">
                    {{ getFileExt(file) }}
                  </span>
                </template>

                <!-- Video Thumbnail with Real Snapshot Preview & Centered Play Overlay -->
                <template v-else-if="isVideo(file)">
                  <video
                    :src="getDownloadUrl(panel.connectionId, file.path) + '#t=0.5'"
                    preload="metadata"
                    muted
                    playsinline
                    class="w-full h-full object-cover group-hover:scale-105 transition duration-300 pointer-events-none rounded-lg"
                  ></video>
                  <div class="absolute inset-0 bg-gradient-to-t from-black/70 via-black/20 to-transparent group-hover:opacity-90 transition"></div>
                  <!-- Centered Play Button -->
                  <div class="absolute inset-0 flex items-center justify-center pointer-events-none">
                    <div class="w-8 h-8 rounded-full bg-black/50 backdrop-blur-md flex items-center justify-center text-white ring-1 ring-white/40 group-hover:scale-110 group-hover:bg-blue-600 transition duration-200 shadow-xl pl-0.5">
                      <FbIcon name="play" size="12px" class="fill-white" />
                    </div>
                  </div>
                  <span class="absolute bottom-1.5 right-1.5 text-[8px] px-1 py-0.2 rounded-md bg-black/75 backdrop-blur-xs text-white/90 font-mono font-bold uppercase tracking-wider shadow-md z-10 border border-white/10">
                    {{ getFileExt(file) }}
                  </span>
                </template>

                <!-- Audio Thumbnail with Music Visual Artwork -->
                <template v-else-if="isAudio(file)">
                  <div class="w-full h-full bg-gradient-to-br from-indigo-500/15 via-purple-500/15 to-pink-500/15 dark:from-indigo-950/50 dark:to-purple-950/50 flex flex-col items-center justify-center space-y-1 rounded-lg">
                    <div class="w-8 h-8 rounded-xl bg-gradient-to-tr from-blue-600 to-indigo-600 flex items-center justify-center text-white text-sm shadow-md group-hover:scale-110 transition duration-200">
                      🎵
                    </div>
                    <span class="text-[8px] font-mono font-bold uppercase text-indigo-600 dark:text-indigo-400 tracking-wider">
                      {{ getFileExt(file) }}
                    </span>
                  </div>
                </template>

                <!-- Dynamic Color-Coded Document / Code / Config / Database / Binary Illustration -->
                <div v-else class="flex flex-col items-center justify-center w-full h-full relative">
                  <div :class="['absolute inset-0 bg-gradient-to-b opacity-60 pointer-events-none rounded-t-2xl', getFileTypeMeta(file).cardBg]"></div>

                  <!-- Sleek Document Illustration -->
                  <div
                    class="w-10 h-13 sm:w-11 sm:h-14 rounded-lg relative flex flex-col items-center justify-between py-1.5 px-1 shadow-xs border group-hover:scale-110 group-hover:shadow-md transition-all duration-200 bg-white/90 dark:bg-slate-900/90"
                    :class="getFileTypeMeta(file).badgeBorder"
                  >
                    <!-- Folded dog-ear corner -->
                    <div
                      class="absolute top-0 right-0 w-2.5 h-2.5 bg-gray-100 dark:bg-slate-950 rounded-bl-md border-l border-b"
                      :class="getFileTypeMeta(file).badgeBorder"
                    ></div>

                    <!-- Category Emoji Symbol -->
                    <span class="text-xs mt-0.5 select-none">{{ getFileTypeMeta(file).symbol }}</span>

                    <!-- Format Pill Badge -->
                    <span
                      class="px-1.5 py-0.5 rounded-md font-mono text-[8px] sm:text-[9px] font-bold uppercase tracking-wider border shadow-2xs max-w-[44px] truncate text-center"
                      :class="[getFileTypeMeta(file).badgeBg, getFileTypeMeta(file).badgeText, getFileTypeMeta(file).badgeBorder]"
                    >
                      {{ getFileTypeMeta(file).label }}
                    </span>
                  </div>
                </div>
              </div>

              <!-- Card Bottom Footer -->
              <div class="p-2 bg-white dark:bg-[#0f1422] shrink-0 text-center flex flex-col items-center justify-center">
                <div class="flex items-center justify-center gap-1 w-full">
                  <span class="font-semibold text-xs text-gray-800 dark:text-slate-100 truncate group-hover:text-blue-600 dark:group-hover:text-blue-400 transition leading-tight block w-full" :title="file.name">
                    {{ file.name }}
                  </span>
                  <span v-if="isItemHidden(file)" class="text-[8px] px-1 py-0.2 rounded bg-gray-200/80 dark:bg-slate-800 text-gray-500 dark:text-slate-400 font-mono shrink-0">
                    dot
                  </span>
                </div>
                <div class="text-[10px] text-gray-400 dark:text-slate-500 mt-0.5 font-normal font-mono truncate w-full">
                  {{ formatBytes(file.size || 0) }}
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- LIST TABLE VIEW -->
      <div v-else class="w-full">
        <table class="w-full text-left border-collapse text-xs select-none">
          <thead class="sticky top-0 z-10 bg-white/95 dark:bg-[#0b0f19]/95 backdrop-blur-xs border-b border-gray-200 dark:border-slate-800 text-[11px] font-bold text-gray-400 dark:text-slate-500 uppercase tracking-wider">
            <tr>
              <th class="py-2.5 px-3 w-8 text-center">
                <input
                  type="checkbox"
                  :checked="isAllSelected"
                  @change="toggleSelectAll"
                  class="rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
                />
              </th>
              <th class="py-2.5 px-2 cursor-pointer hover:text-gray-900 dark:hover:text-white transition" @click="setSort('name')">
                <div class="flex items-center space-x-1">
                  <span>Name</span>
                  <span v-if="panel.sortField === 'name'" class="text-blue-600 dark:text-blue-400 text-xs">
                    {{ panel.sortOrder === 'asc' ? '▲' : '▼' }}
                  </span>
                </div>
              </th>
              <th class="py-2.5 px-2 w-28 text-right cursor-pointer hover:text-gray-900 dark:hover:text-white transition" @click="setSort('size')">
                <div class="flex items-center justify-end space-x-1">
                  <span>Size</span>
                  <span v-if="panel.sortField === 'size'" class="text-blue-600 dark:text-blue-400 text-xs">
                    {{ panel.sortOrder === 'asc' ? '▲' : '▼' }}
                  </span>
                </div>
              </th>
              <th class="py-2.5 px-2 w-36 text-right cursor-pointer hover:text-gray-900 dark:hover:text-white transition" @click="setSort('modified')">
                <div class="flex items-center justify-end space-x-1">
                  <span>Modified</span>
                  <span v-if="panel.sortField === 'modified'" class="text-blue-600 dark:text-blue-400 text-xs">
                    {{ panel.sortOrder === 'asc' ? '▲' : '▼' }}
                  </span>
                </div>
              </th>
            </tr>
          </thead>
          <tbody class="divide-y divide-gray-100 dark:divide-slate-800/60 font-sans">
            <!-- Parent Folder Navigation Row (..) -->
            <tr
              v-if="panel.path !== '/' && panel.path !== ''"
              @click="workspaceStore.navigateUp(panelId)"
              @dblclick="workspaceStore.navigateUp(panelId)"
              class="cursor-pointer transition hover:bg-blue-50/40 dark:hover:bg-slate-800/60 text-gray-700 dark:text-slate-300"
              title="Go to parent directory (..)"
            >
              <td class="py-2.5 px-3 text-center"></td>
              <td class="py-2.5 px-2 flex items-center space-x-3 truncate">
                <FbIcon name="chevron-left" size="16px" class="text-blue-500 shrink-0" />
                <span class="font-bold text-sm text-blue-600 dark:text-blue-400">.. (Parent Directory)</span>
              </td>
              <td class="py-2.5 px-2 text-right text-gray-400 font-mono text-xs">-</td>
              <td class="py-2.5 px-2 text-right text-gray-400 font-mono text-xs">-</td>
            </tr>

            <tr
              v-for="entry in displayedEntries"
              :key="entry.path"
              data-entry-item="true"
              :data-entry-path="entry.path"
              draggable="true"
              @dragstart="handleDragStart($event, entry)"
              @touchstart.passive="handleTouchStart($event, entry)"
              @touchend="handleTouchEnd"
              @touchmove="handleTouchMove"
              @touchcancel="handleTouchEnd"
              @click="handleEntryClick($event, entry)"
              @dblclick="handleEntryDoubleClick(entry)"
              @contextmenu="openContextMenu($event, entry)"
              @dragover.stop.prevent="entry.kind === 'directory' ? handleFolderDragOver($event, entry) : null"
              @dragleave.stop="entry.kind === 'directory' ? handleFolderDragLeave(entry) : null"
              @drop.stop.prevent="entry.kind === 'directory' ? handleDrop($event, entry) : null"
              :class="[
                'cursor-pointer transition group',
                isItemHidden(entry) ? 'opacity-65 hover:opacity-100 italic' : '',
                workspaceStore.isCutItem(panel.connectionId, entry.path) ? 'opacity-40 italic' : '',
                hoveredFolderDrop === entry.path
                  ? 'bg-blue-100/70 dark:bg-blue-900/60 border-l-4 border-l-blue-600'
                  : (panel.selectedEntries.includes(entry.path)
                    ? 'bg-blue-50/80 dark:bg-blue-950/40 text-blue-900 dark:text-blue-200 border-l-2 border-l-blue-600 dark:border-l-blue-400'
                    : 'hover:bg-gray-50/80 dark:hover:bg-slate-800/60 text-gray-800 dark:text-slate-200 border-l-2 border-l-transparent')
              ]"
            >
              <td
                :class="[
                  uiStore.listDensity === 'comfortable' ? 'py-3' : (uiStore.listDensity === 'dense' ? 'py-1' : 'py-2'),
                  'px-3 text-center'
                ]"
                @click.stop
              >
                <input
                  type="checkbox"
                  :checked="panel.selectedEntries.includes(entry.path)"
                  @change="toggleEntrySelect(entry.path, true)"
                  class="rounded bg-white dark:bg-slate-900 border-gray-300 dark:border-slate-700 text-blue-600 focus:ring-0 cursor-pointer"
                />
              </td>
              <td
                :class="[
                  uiStore.listDensity === 'comfortable' ? 'py-3' : (uiStore.listDensity === 'dense' ? 'py-1' : 'py-2'),
                  'px-2 flex items-center space-x-3 truncate'
                ]"
              >
                <FbIcon
                  :name="entry.kind === 'directory' ? 'folder' : getCategoryIcon(entry)"
                  :size="uiStore.listDensity === 'dense' ? '15px' : '18px'"
                  :class="isItemHidden(entry) ? 'text-gray-400 dark:text-slate-500' : (entry.kind === 'directory' ? 'text-blue-600 dark:text-blue-400' : 'text-gray-400 dark:text-slate-500')"
                />
                <div class="truncate flex items-center space-x-1.5">
                  <span
                    class="truncate font-medium group-hover:text-blue-600 dark:group-hover:text-blue-400 transition"
                    :class="[
                      entry.kind === 'directory' ? 'font-semibold' : '',
                      uiStore.listDensity === 'dense' ? 'text-xs' : 'text-sm'
                    ]"
                  >
                    {{ entry.name }}
                  </span>
                  <span v-if="isItemHidden(entry)" class="text-[9px] px-1.5 py-0.2 rounded bg-gray-200/80 dark:bg-slate-800 text-gray-400 dark:text-slate-500 font-mono not-italic">
                    dot
                  </span>
                </div>
              </td>
              <td
                :class="[
                  uiStore.listDensity === 'comfortable' ? 'py-3' : (uiStore.listDensity === 'dense' ? 'py-1' : 'py-2'),
                  'px-2 text-right text-gray-500 dark:text-slate-400 font-mono text-xs'
                ]"
              >
                {{ entry.kind === 'directory' ? '—' : formatBytes(entry.size || 0) }}
              </td>
              <td
                :class="[
                  uiStore.listDensity === 'comfortable' ? 'py-3' : (uiStore.listDensity === 'dense' ? 'py-1' : 'py-2'),
                  'px-2 text-right text-gray-400 dark:text-slate-500 text-xs truncate'
                ]"
              >
                {{ formatDate(entry.modified_at) }}
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- Floating Contextual Selection Action Bar -->
    <FilePanelSelectionBar
      :selected-count="panel.selectedEntries.length"
      :selected-size="selectedTotalSize"
      :single-selected="panel.selectedEntries.length === 1"
      @compress="handleBatchCompress"
      @rename="handleSingleRename"
      @delete="handleBatchDelete"
      @clear="panel.selectedEntries = []"
    />

    <!-- Information-Dense Panel Status Bar -->
    <FilePanelStatusBar
      :displayed-count="displayedEntries.length"
      :selected-count="panel.selectedEntries.length"
      :selected-size="selectedTotalSize"
      :total-folder-size="totalFolderSize"
      :view-mode="panel.viewMode"
      :stale="panel.stale"
      :error="!!panel.error"
      :current-conn-name="currentConnName"
      :is-read-only="connStore.isReadOnly(panel.connectionId)"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref, nextTick, onMounted, onUnmounted } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import FilePanelSelectionBar from './FilePanelSelectionBar.vue';
import FilePanelStatusBar from './FilePanelStatusBar.vue';
import type { IconName } from '../../utils/icons';
import { getFileTypeMeta, getFileExt, isTextOrCode } from '../../utils/fileTypes';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useConnectionStore } from '../../stores/connectionStore';
import { useFileStore } from '../../stores/fileStore';
import { useTransferStore } from '../../stores/transferStore';
import { useUiStore } from '../../stores/uiStore';
import { getDownloadUrl, uploadFileApi } from '../../api/files';
import type { FileEntry } from '../../types/vfs';
import { PreviewResolver } from '../../services/previewResolver';

const props = defineProps<{
  panelId: 'left' | 'right';
}>();

const emit = defineEmits<{
  (e: 'openArchiveViewer', payload: { connectionId: string; path: string }): void;
}>();

const workspaceStore = useWorkspaceStore();
const connStore = useConnectionStore();
const fileStore = useFileStore();
const transferStore = useTransferStore();
const uiStore = useUiStore();

const panel = computed(() => workspaceStore.getPanel(props.panelId));
const isActive = computed(() => workspaceStore.activePanelId === props.panelId);
const isDragOver = ref(false);

const isAddressBar = ref(false);
const addressInput = ref('');
const addressInputRef = ref<HTMLInputElement | null>(null);

const currentConnName = computed(() => {
  const c = connStore.connections.find((x) => x.id === panel.value.connectionId);
  return c ? c.name : panel.value.connectionId;
});

async function openAddressBar() {
  addressInput.value = panel.value.path;
  isAddressBar.value = true;
  await nextTick();
  addressInputRef.value?.focus();
  addressInputRef.value?.select();
}

async function submitAddressBar() {
  const target = addressInput.value.trim();
  if (target) {
    isAddressBar.value = false;
    await workspaceStore.navigateTo(props.panelId, target.startsWith('/') ? target : `/${target}`);
  } else {
    isAddressBar.value = false;
  }
}

const breadcrumbSegments = computed(() => {
  const p = panel.value.path || '/';
  if (p === '/' || p === '') return [];
  const parts = p.split('/').filter(Boolean);
  let currentAccum = '';
  return parts.map((name) => {
    currentAccum += `/${name}`;
    return {
      name,
      path: currentAccum,
    };
  });
});

const pathSuggestions = computed(() => {
  if (!isAddressBar.value || !addressInput.value) return [];
  const currentText = addressInput.value.toLowerCase();
  return panel.value.entries
    .filter((e) => e.kind === 'directory' && e.path.toLowerCase().includes(currentText))
    .map((e) => e.path)
    .slice(0, 5);
});

function applySuggestion(sug: string) {
  addressInput.value = sug;
  submitAddressBar();
}

function autocompleteFirstPath() {
  if (pathSuggestions.value.length > 0) {
    applySuggestion(pathSuggestions.value[0]);
  }
}

function handleAddressBlur() {
  setTimeout(() => {
    isAddressBar.value = false;
  }, 200);
}

const selectedTotalSize = computed(() => {
  const selectedSet = new Set(panel.value.selectedEntries);
  return panel.value.entries
    .filter((e: FileEntry) => selectedSet.has(e.path))
    .reduce((acc: number, curr: FileEntry) => acc + (curr.size || 0), 0);
});

const totalFolderSize = computed(() => {
  return panel.value.entries.reduce((acc: number, curr: FileEntry) => acc + (curr.size || 0), 0);
});

const displayedEntries = computed(() => {
  let list = [...panel.value.entries];

  // 1. Filter by category (if filterType is active)
  if (panel.value.filterType && panel.value.filterType !== 'all') {
    switch (panel.value.filterType) {
      case 'folders':
        list = list.filter((e) => e.kind === 'directory');
        break;
      case 'images':
        list = list.filter((e) => e.kind !== 'directory' && isImage(e));
        break;
      case 'videos':
        list = list.filter((e) => e.kind !== 'directory' && isVideo(e));
        break;
      case 'audio':
        list = list.filter((e) => e.kind !== 'directory' && isAudio(e));
        break;
      case 'code':
        list = list.filter((e) => e.kind !== 'directory' && isTextOrCode(e));
        break;
      case 'archives':
        list = list.filter((e) => e.kind !== 'directory' && ['zip', 'tar', 'gz', 'tgz', '7z', 'rar'].includes(getFileExt(e)));
        break;
    }
  }

  // 2. Sort entries (Client-side fast sort with natural numbers)
  const field = panel.value.sortField || 'name';
  const order = panel.value.sortOrder === 'desc' ? -1 : 1;

  list.sort((a, b) => {
    // Folders on top
    if (a.kind === 'directory' && b.kind !== 'directory') return -1;
    if (a.kind !== 'directory' && b.kind === 'directory') return 1;

    if (field === 'size') {
      const sizeA = a.size || 0;
      const sizeB = b.size || 0;
      return (sizeA - sizeB) * order;
    } else if (field === 'modified') {
      const dateA = a.modified_at ? new Date(a.modified_at).getTime() : 0;
      const dateB = b.modified_at ? new Date(b.modified_at).getTime() : 0;
      return (dateA - dateB) * order;
    } else if (field === 'type') {
      const extA = getFileExt(a);
      const extB = getFileExt(b);
      return extA.localeCompare(extB) * order;
    } else {
      // Natural numeric sort (e.g. 1.mp4, 2.mp4, 10.mp4)
      return a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' }) * order;
    }
  });

  return list;
});

const displayedFolders = computed(() => {
  return displayedEntries.value.filter((e) => e.kind === 'directory');
});

const displayedFiles = computed(() => {
  return displayedEntries.value.filter((e) => e.kind !== 'directory');
});

const isAllSelected = computed(() => {
  if (displayedEntries.value.length === 0) return false;
  return displayedEntries.value.every((e) => panel.value.selectedEntries.includes(e.path));
});

onMounted(async () => {
  await workspaceStore.fetchPanelEntries(props.panelId);
});

function isItemHidden(entry: FileEntry): boolean {
  return entry.is_hidden || entry.name.startsWith('.');
}

function isImage(entry: FileEntry): boolean {
  const ext = getFileExt(entry);
  return ['jpg', 'jpeg', 'png', 'gif', 'webp', 'svg', 'ico'].includes(ext);
}

function isVideo(entry: FileEntry): boolean {
  const ext = getFileExt(entry);
  return ['mp4', 'webm', 'mov', 'avi', 'mkv', 'flv'].includes(ext);
}

function isAudio(entry: FileEntry): boolean {
  const ext = getFileExt(entry);
  return ['mp3', 'wav', 'flac', 'aac', 'm4a', 'opus', 'ogg'].includes(ext);
}

function getCategoryIcon(entry: FileEntry): IconName {
  const ext = getFileExt(entry);
  if (['zip', 'tar', 'gz', 'tgz', '7z', 'rar'].includes(ext)) return 'archive';
  if (['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'ico'].includes(ext)) return 'image';
  if (['mp4', 'webm', 'mov', 'mkv', 'avi'].includes(ext)) return 'video';
  if (['mp3', 'wav', 'ogg', 'flac', 'm4a'].includes(ext)) return 'audio';
  if (['pdf'].includes(ext)) return 'pdf';
  if (isTextOrCode(entry)) return 'code';
  return 'file';
}

function toggleSelectAll() {
  if (isAllSelected.value) {
    panel.value.selectedEntries = [];
  } else {
    panel.value.selectedEntries = displayedEntries.value.map((e) => e.path);
  }
}

function setSort(field: string) {
  if (panel.value.sortField === field) {
    panel.value.sortOrder = panel.value.sortOrder === 'asc' ? 'desc' : 'asc';
  } else {
    panel.value.sortField = field;
    panel.value.sortOrder = 'asc';
  }
  workspaceStore.fetchPanelEntries(props.panelId);
}

function toggleEntrySelect(path: string, multi: boolean = false) {
  if (!multi) {
    panel.value.selectedEntries = panel.value.selectedEntries.includes(path) ? [] : [path];
  } else {
    if (panel.value.selectedEntries.includes(path)) {
      panel.value.selectedEntries = panel.value.selectedEntries.filter((p: string) => p !== path);
    } else {
      panel.value.selectedEntries.push(path);
    }
  }
}

let lastClickedIndex = -1;
let touchTimer: any = null;
let touchMoved = false;
let lastClickTime = 0;
let lastClickPath = '';

const panelContentRef = ref<HTMLElement | null>(null);
const isPullRefreshing = ref(false);
const pullDistance = ref(0);
let startY = 0;
let isPulling = false;

function getGridColumnCount(): number {
  if (panel.value.viewMode !== 'grid' || !panelContentRef.value) return 1;
  const items = panelContentRef.value.querySelectorAll('[data-entry-item="true"]');
  if (!items || items.length < 2) return 1;
  const firstTop = (items[0] as HTMLElement).offsetTop;
  let count = 0;
  for (let i = 0; i < items.length; i++) {
    if ((items[i] as HTMLElement).offsetTop === firstTop) {
      count++;
    } else {
      break;
    }
  }
  return Math.max(1, count);
}

function selectAndScrollToIndex(idx: number, isRange: boolean = false) {
  if (idx < 0 || idx >= displayedEntries.value.length) return;
  const targetEntry = displayedEntries.value[idx];
  if (isRange) {
    const anchor = lastClickedIndex !== -1 ? lastClickedIndex : idx;
    const start = Math.min(anchor, idx);
    const end = Math.max(anchor, idx);
    panel.value.selectedEntries = displayedEntries.value.slice(start, end + 1).map((i: FileEntry) => i.path);
  } else {
    panel.value.selectedEntries = [targetEntry.path];
    lastClickedIndex = idx;
  }

  nextTick(() => {
    const el = panelContentRef.value?.querySelector(`[data-entry-path="${CSS.escape(targetEntry.path)}"]`);
    if (el) {
      (el as HTMLElement).scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    }
  });
}

function onContainerTouchStart(e: TouchEvent) {
  if (!uiStore.isMobile || !panelContentRef.value) return;
  if (panelContentRef.value.scrollTop <= 0) {
    startY = e.touches[0].clientY;
    isPulling = true;
  }
}

function onContainerTouchMove(e: TouchEvent) {
  if (!isPulling || isPullRefreshing.value || !panelContentRef.value) return;
  if (panelContentRef.value.scrollTop <= 0) {
    const currentY = e.touches[0].clientY;
    const diff = currentY - startY;
    if (diff > 0) {
      pullDistance.value = Math.min(70, diff * 0.45);
    }
  } else {
    isPulling = false;
    pullDistance.value = 0;
  }
}

async function onContainerTouchEnd() {
  if (pullDistance.value >= 40 && !isPullRefreshing.value) {
    isPullRefreshing.value = true;
    pullDistance.value = 45;
    try {
      await workspaceStore.fetchPanelEntries(props.panelId);
    } finally {
      setTimeout(() => {
        isPullRefreshing.value = false;
        pullDistance.value = 0;
        isPulling = false;
      }, 250);
    }
  } else {
    pullDistance.value = 0;
    isPulling = false;
  }
}

function triggerHaptic(duration: number = 35) {
  if (typeof navigator !== 'undefined' && 'vibrate' in navigator) {
    try {
      navigator.vibrate(duration);
    } catch (_) {}
  }
}

function handleTouchStart(_e: TouchEvent, entry: FileEntry) {
  touchMoved = false;
  if (touchTimer) clearTimeout(touchTimer);
  touchTimer = setTimeout(() => {
    if (!touchMoved) {
      triggerHaptic(40);
      // Long press triggers bottom sheet context menu and selects item
      if (!panel.value.selectedEntries.includes(entry.path)) {
        panel.value.selectedEntries = [entry.path];
      }
      uiStore.openContextMenuForTouch(entry, panel.value.connectionId, props.panelId);
    }
  }, 450);
}

function handleTouchMove() {
  touchMoved = true;
  if (touchTimer) clearTimeout(touchTimer);
}

function handleTouchEnd() {
  if (touchTimer) clearTimeout(touchTimer);
}

function handleEntryClick(e: MouseEvent, entry: FileEntry) {
  if (uiStore.isMobile) {
    if (panel.value.selectedEntries.length > 0) {
      // In selection mode on mobile: tap toggles item selection
      toggleEntrySelect(entry.path, true);
    } else {
      // Not in selection mode: single tap opens directly!
      handleEntryDoubleClick(entry);
    }
    return;
  }

  const now = Date.now();
  const isQuickSecondClick = (now - lastClickTime < 380) && (lastClickPath === entry.path);
  lastClickTime = now;
  lastClickPath = entry.path;

  if (isQuickSecondClick && !e.shiftKey && !e.ctrlKey && !e.metaKey) {
    handleEntryDoubleClick(entry);
    return;
  }

  const currentIndex = displayedEntries.value.findIndex((item: FileEntry) => item.path === entry.path);

  if (e.shiftKey && currentIndex !== -1) {
    const anchor = lastClickedIndex !== -1 ? lastClickedIndex : 0;
    const start = Math.min(anchor, currentIndex);
    const end = Math.max(anchor, currentIndex);
    const rangePaths = displayedEntries.value.slice(start, end + 1).map((item: FileEntry) => item.path);
    if (e.ctrlKey || e.metaKey) {
      panel.value.selectedEntries = Array.from(new Set([...panel.value.selectedEntries, ...rangePaths]));
    } else {
      panel.value.selectedEntries = rangePaths;
    }
  } else if (e.ctrlKey || e.metaKey) {
    if (panel.value.selectedEntries.includes(entry.path)) {
      panel.value.selectedEntries = panel.value.selectedEntries.filter((p: string) => p !== entry.path);
    } else {
      panel.value.selectedEntries.push(entry.path);
    }
    lastClickedIndex = currentIndex;
  } else {
    panel.value.selectedEntries = [entry.path];
    lastClickedIndex = currentIndex;
  }
}

function handleContainerClick(e: MouseEvent) {
  const target = e.target as HTMLElement;
  if (
    target.closest('[data-entry-item]') ||
    target.closest('button') ||
    target.closest('input') ||
    target.closest('a') ||
    target.closest('th')
  ) {
    return;
  }

  // User clicked on blank/empty space of workspace -> clear selection!
  workspaceStore.setActivePanel(props.panelId);
  panel.value.selectedEntries = [];
  lastClickedIndex = -1;
  uiStore.closeContextMenu();
}

let lastOpenTime = 0;
let lastOpenPath = '';

async function handleEntryDoubleClick(entry: FileEntry) {
  const now = Date.now();
  if (now - lastOpenTime < 350 && lastOpenPath === entry.path) {
    return;
  }
  lastOpenTime = now;
  lastOpenPath = entry.path;

  if (entry.kind === 'directory') {
    panel.value.selectedEntries = [];
    lastClickedIndex = -1;
    await workspaceStore.navigatePanel(props.panelId, entry.path);
    return;
  }

  const resolution = PreviewResolver.resolve(
    entry,
    panel.value.connectionId,
    displayedFiles.value,
    (payload) => emit('openArchiveViewer', payload)
  );

  await resolution.open();
}

let dragEnterCounter = 0;

function handleDragEnter(e: DragEvent) {
  e.preventDefault();
  dragEnterCounter++;
  isDragOver.value = true;
}

function handleDragLeave(e: DragEvent) {
  e.preventDefault();
  dragEnterCounter--;
  if (dragEnterCounter <= 0) {
    dragEnterCounter = 0;
    isDragOver.value = false;
  }
}

const hoveredFolderDrop = ref<string | null>(null);

function handleFolderDragOver(e: DragEvent, folder: FileEntry) {
  e.preventDefault();
  hoveredFolderDrop.value = folder.path;
  if (e.dataTransfer) {
    e.dataTransfer.dropEffect = e.shiftKey ? 'move' : 'copy';
  }
}

function handleFolderDragLeave(folder: FileEntry) {
  if (hoveredFolderDrop.value === folder.path) {
    hoveredFolderDrop.value = null;
  }
}

function handleDragOver(e: DragEvent) {
  e.preventDefault();
  if (e.dataTransfer) {
    e.dataTransfer.dropEffect = e.shiftKey ? 'move' : 'copy';
  }
}

function openContextMenu(e: MouseEvent, entry: FileEntry) {
  fileStore.currentConnectionId = panel.value.connectionId;
  fileStore.currentPath = panel.value.path;
  workspaceStore.activePanelId = props.panelId;

  if (!panel.value.selectedEntries.includes(entry.path)) {
    panel.value.selectedEntries = [entry.path];
  }

  uiStore.openContextMenu(e, entry, panel.value.connectionId, props.panelId);
}

function openBlankContextMenu(e: MouseEvent) {
  fileStore.currentConnectionId = panel.value.connectionId;
  fileStore.currentPath = panel.value.path;
  workspaceStore.activePanelId = props.panelId;
  uiStore.openContextMenu(e, null, panel.value.connectionId, props.panelId);
}

function handleBatchCompress() {
  fileStore.currentConnectionId = panel.value.connectionId;
  fileStore.currentPath = panel.value.path;
}

function handleSingleRename() {
  const selectedPath = panel.value.selectedEntries[0];
  const selectedEntry = panel.value.entries.find((e) => e.path === selectedPath);
  if (selectedEntry) {
    fileStore.currentConnectionId = panel.value.connectionId;
    fileStore.currentPath = panel.value.path;
    uiStore.openRename(selectedEntry);
  }
}

function handleBatchDelete() {
  fileStore.currentConnectionId = panel.value.connectionId;
  fileStore.currentPath = panel.value.path;
  uiStore.openDelete(panel.value.selectedEntries);
}

function handleDragStart(e: DragEvent, entry: FileEntry) {
  const selected = panel.value.selectedEntries.includes(entry.path)
    ? panel.value.selectedEntries
    : [entry.path];

  if (!panel.value.selectedEntries.includes(entry.path)) {
    panel.value.selectedEntries = [entry.path];
  }

  const payload = {
    sourcePanelId: props.panelId,
    sourceConnectionId: panel.value.connectionId,
    paths: selected,
  };

  const payloadStr = JSON.stringify(payload);
  e.dataTransfer?.setData('application/json', payloadStr);
  e.dataTransfer?.setData('text/plain', payloadStr);
  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = 'copyMove';
  }
}

interface DroppedUploadItem {
  file: File;
  relativePath: string;
}

async function traverseDirectoryEntry(entry: any, currentPath: string): Promise<DroppedUploadItem[]> {
  const items: DroppedUploadItem[] = [];
  if (entry.isFile) {
    const file: File = await new Promise((resolve, reject) => entry.file(resolve, reject));
    items.push({
      file,
      relativePath: currentPath ? `${currentPath}/${entry.name}` : entry.name,
    });
  } else if (entry.isDirectory) {
    const dirReader = entry.createReader();
    const readAllEntries = async (): Promise<any[]> => {
      const entries: any[] = [];
      let batch: any[] = [];
      do {
        batch = await new Promise((resolve, reject) => dirReader.readEntries(resolve, reject));
        entries.push(...batch);
      } while (batch.length > 0);
      return entries;
    };

    const dirEntries = await readAllEntries();
    const nextPath = currentPath ? `${currentPath}/${entry.name}` : entry.name;
    for (const child of dirEntries) {
      const childItems = await traverseDirectoryEntry(child, nextPath);
      items.push(...childItems);
    }
  }
  return items;
}

async function handleExternalFilesDrop(e: DragEvent, targetDir: string) {
  const items = e.dataTransfer?.items;
  const files = e.dataTransfer?.files;
  if (!items && !files) return;

  const uploadItems: DroppedUploadItem[] = [];

  if (items && items.length > 0) {
    for (let i = 0; i < items.length; i++) {
      const item = items[i];
      if (item.webkitGetAsEntry) {
        const entry = item.webkitGetAsEntry();
        if (entry) {
          const entryItems = await traverseDirectoryEntry(entry, '');
          uploadItems.push(...entryItems);
        }
      } else if (item.kind === 'file') {
        const f = item.getAsFile();
        if (f) {
          uploadItems.push({ file: f, relativePath: f.name });
        }
      }
    }
  } else if (files && files.length > 0) {
    for (let i = 0; i < files.length; i++) {
      const f = files[i];
      uploadItems.push({ file: f, relativePath: f.name });
    }
  }

  if (uploadItems.length === 0) return;

  uiStore.showToast(`Uploading ${uploadItems.length} file(s)...`, 'info');
  const connId = panel.value.connectionId || 'local';

  // Upload in bounded parallel batches (2 workers)
  const concurrency = 2;
  let nextIdx = 0;
  let successCount = 0;

  async function worker() {
    while (nextIdx < uploadItems.length) {
      const item = uploadItems[nextIdx++];
      const cleanRel = item.relativePath.replace(/^\/+/, '');
      const fullDest = targetDir === '/' ? `/${cleanRel}` : `${targetDir}/${cleanRel}`;
      try {
        await uploadFileApi(connId, fullDest, item.file);
        successCount++;
      } catch (err) {
        console.error('Failed uploading item', cleanRel, err);
      }
    }
  }

  const workers = [];
  for (let w = 0; w < Math.min(concurrency, uploadItems.length); w++) {
    workers.push(worker());
  }
  await Promise.all(workers);

  uiStore.showToast(`Uploaded ${successCount} file(s) to ${targetDir}`, 'success');
  await workspaceStore.fetchPanelEntries(props.panelId);
}

async function handleDrop(e: DragEvent, targetFolder?: FileEntry) {
  dragEnterCounter = 0;
  isDragOver.value = false;
  hoveredFolderDrop.value = null;

  const targetDir = targetFolder && targetFolder.kind === 'directory'
    ? targetFolder.path
    : panel.value.path;

  const rawData = e.dataTransfer?.getData('application/json') || e.dataTransfer?.getData('text/plain');
  if (!rawData) {
    // OS File / Folder drop
    await handleExternalFilesDrop(e, targetDir);
    return;
  }

  try {
    const data = JSON.parse(rawData);
    if (!data.paths || data.paths.length === 0) return;

    // Prevent dropping into the exact same folder on the same connection
    if (data.sourceConnectionId === panel.value.connectionId && data.paths.every((p: string) => {
      const parent = p.substring(0, p.lastIndexOf('/')) || '/';
      return parent === targetDir;
    })) {
      return;
    }

    const isMove = e.shiftKey || (data.sourceConnectionId === panel.value.connectionId && !e.ctrlKey && !e.altKey);
    const opType: 'copy' | 'move' = isMove ? 'move' : 'copy';
    const opLabel = isMove ? 'Move' : 'Copy';

    for (const filePath of data.paths) {
      let fileName = filePath.split('/').pop() || 'file';
      let targetPath = targetDir === '/' ? `/${fileName}` : `${targetDir}/${fileName}`;

      // Check if target directory already has an entry with the same name
      const alreadyExists = panel.value.entries.some((e: FileEntry) => e.name === fileName);
      if (alreadyExists) {
        const resolution = await transferStore.requestConflict(fileName, filePath, targetPath);
        if (resolution === 'cancel') {
          break;
        }
        if (resolution === 'skip') {
          continue;
        }
        if (resolution === 'keep_both') {
          const dotIdx = fileName.lastIndexOf('.');
          let count = 1;
          let candidateName = dotIdx > 0
            ? `${fileName.substring(0, dotIdx)} (${count})${fileName.substring(dotIdx)}`
            : `${fileName} (${count})`;

          while (panel.value.entries.some((e: FileEntry) => e.name === candidateName)) {
            count++;
            candidateName = dotIdx > 0
              ? `${fileName.substring(0, dotIdx)} (${count})${fileName.substring(dotIdx)}`
              : `${fileName} (${count})`;
          }

          fileName = candidateName;
          targetPath = targetDir === '/' ? `/${fileName}` : `${targetDir}/${fileName}`;
        }
      }

      await transferStore.submitTransfer(
        `${opLabel} ${fileName} to ${targetDir}`,
        opType,
        data.sourceConnectionId,
        filePath,
        panel.value.connectionId,
        targetPath
      );
    }
    uiStore.showToast(`Queued ${data.paths.length} ${opLabel.toLowerCase()}(s)`, 'info');
    setTimeout(() => {
      workspaceStore.fetchPanelEntries(props.panelId);
      if (isMove && data.sourcePanelId && data.sourcePanelId !== props.panelId) {
        workspaceStore.fetchPanelEntries(data.sourcePanelId);
      }
    }, 1000);
  } catch (err: any) {
    uiStore.showToast('Transfer queue failed', 'error');
  }
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
}

function formatDate(dateStr?: string): string {
  if (!dateStr) return '—';
  const d = new Date(dateStr);
  return d.toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
}

function handleKeyDown(e: KeyboardEvent) {
  if (!isActive.value) return;
  // Ignore keyboard shortcuts when typing in inputs or dialogs
  const tag = (e.target as HTMLElement)?.tagName;
  if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;

  if (e.key === 'Tab' && workspaceStore.isDualPane) {
    e.preventDefault();
    workspaceStore.setActivePanel(props.panelId === 'left' ? 'right' : 'left');
    return;
  } else if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'ArrowLeft') {
    e.preventDefault();
    workspaceStore.setActivePanel('left');
    return;
  } else if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'ArrowRight') {
    e.preventDefault();
    workspaceStore.setActivePanel('right');
    return;
  } else if (e.key === 'F5') {
    e.preventDefault();
    workspaceStore.fetchPanelEntries(props.panelId);
    return;
  } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'l') {
    e.preventDefault();
    openAddressBar();
  } else if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
    e.preventDefault();
    if (panel.value.selectedEntries.length === 1) {
      const selected = panel.value.entries.find((i) => i.path === panel.value.selectedEntries[0]);
      if (selected && selected.kind === 'directory') {
        workspaceStore.openInOtherPanel(props.panelId, selected.path);
      }
    }
  } else if ((e.ctrlKey || e.metaKey) && e.key === 'a') {
    e.preventDefault();
    panel.value.selectedEntries = panel.value.entries.map((item) => item.path);
  } else if ((e.ctrlKey || e.metaKey) && e.key === 'c') {
    e.preventDefault();
    workspaceStore.copySelection(props.panelId);
    uiStore.showToast('Copied to clipboard', 'info');
  } else if ((e.ctrlKey || e.metaKey) && e.key === 'x') {
    e.preventDefault();
    workspaceStore.cutSelection(props.panelId);
    uiStore.showToast('Cut to clipboard', 'info');
  } else if ((e.ctrlKey || e.metaKey) && e.key === 'v') {
    e.preventDefault();
    workspaceStore.paste(props.panelId);
  } else if (e.key === 'Delete') {
    e.preventDefault();
    if (panel.value.selectedEntries.length > 0) {
      handleBatchDelete();
    }
  } else if (e.key === 'F2') {
    e.preventDefault();
    if (panel.value.selectedEntries.length === 1) {
      handleSingleRename();
    }
  } else if (e.altKey && e.key === 'ArrowLeft') {
    e.preventDefault();
    workspaceStore.goBack(props.panelId);
  } else if (e.altKey && e.key === 'ArrowRight') {
    e.preventDefault();
    workspaceStore.goForward(props.panelId);
  } else if (e.altKey && e.key === 'ArrowUp') {
    e.preventDefault();
    workspaceStore.navigateUp(props.panelId);
  } else if (e.key === 'Enter') {
    if (panel.value.selectedEntries.length === 1) {
      const selected = panel.value.entries.find((i) => i.path === panel.value.selectedEntries[0]);
      if (selected) {
        handleEntryDoubleClick(selected);
      }
    }
  } else if (e.key === 'ArrowRight') {
    e.preventDefault();
    if (displayedEntries.value.length === 0) return;
    if (panel.value.viewMode === 'grid') {
      if (panel.value.selectedEntries.length === 0) {
        selectAndScrollToIndex(0, e.shiftKey);
      } else {
        const lastSelectedPath = panel.value.selectedEntries[panel.value.selectedEntries.length - 1];
        const idx = displayedEntries.value.findIndex((i: FileEntry) => i.path === lastSelectedPath);
        const nextIdx = Math.min(displayedEntries.value.length - 1, (idx >= 0 ? idx : 0) + 1);
        selectAndScrollToIndex(nextIdx, e.shiftKey);
      }
    } else {
      if (panel.value.selectedEntries.length === 1) {
        const selected = panel.value.entries.find((i) => i.path === panel.value.selectedEntries[0]);
        if (selected) {
          handleEntryDoubleClick(selected);
        }
      } else if (panel.value.selectedEntries.length === 0) {
        selectAndScrollToIndex(0, false);
      }
    }
  } else if (e.key === 'ArrowLeft') {
    e.preventDefault();
    if (displayedEntries.value.length === 0) return;
    if (panel.value.viewMode === 'grid') {
      if (panel.value.selectedEntries.length === 0) {
        selectAndScrollToIndex(displayedEntries.value.length - 1, e.shiftKey);
      } else {
        const firstSelectedPath = panel.value.selectedEntries[0];
        const idx = displayedEntries.value.findIndex((i: FileEntry) => i.path === firstSelectedPath);
        const prevIdx = Math.max(0, (idx >= 0 ? idx : 0) - 1);
        selectAndScrollToIndex(prevIdx, e.shiftKey);
      }
    } else {
      if (panel.value.path !== '/' && panel.value.path !== '') {
        workspaceStore.navigateUp(props.panelId);
      }
    }
  } else if (e.key === 'ArrowDown') {
    e.preventDefault();
    if (displayedEntries.value.length === 0) return;
    const step = panel.value.viewMode === 'grid' ? getGridColumnCount() : 1;
    if (panel.value.selectedEntries.length === 0) {
      selectAndScrollToIndex(0, e.shiftKey);
    } else {
      const lastSelectedPath = panel.value.selectedEntries[panel.value.selectedEntries.length - 1];
      const idx = displayedEntries.value.findIndex((i: FileEntry) => i.path === lastSelectedPath);
      const nextIdx = Math.min(displayedEntries.value.length - 1, (idx >= 0 ? idx : 0) + step);
      selectAndScrollToIndex(nextIdx, e.shiftKey);
    }
  } else if (e.key === 'ArrowUp') {
    e.preventDefault();
    if (displayedEntries.value.length === 0) return;
    const step = panel.value.viewMode === 'grid' ? getGridColumnCount() : 1;
    if (panel.value.selectedEntries.length === 0) {
      selectAndScrollToIndex(displayedEntries.value.length - 1, e.shiftKey);
    } else {
      const firstSelectedPath = panel.value.selectedEntries[0];
      const idx = displayedEntries.value.findIndex((i: FileEntry) => i.path === firstSelectedPath);
      const prevIdx = Math.max(0, (idx >= 0 ? idx : 0) - step);
      selectAndScrollToIndex(prevIdx, e.shiftKey);
    }
  } else if (e.key === 'Backspace') {
    e.preventDefault();
    workspaceStore.navigateUp(props.panelId);
  } else if (e.key === 'Escape') {
    panel.value.selectedEntries = [];
  }
}

onMounted(() => {
  window.addEventListener('keydown', handleKeyDown);
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown);
});
</script>

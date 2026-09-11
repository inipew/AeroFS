<template>
  <Transition :name="menuTransitionName">
    <div v-if="uiStore.contextMenu.visible" class="fixed inset-0 pointer-events-none z-50">
      <!-- Mobile Backdrop Overlay -->
      <div
        v-if="uiStore.isMobile"
        @click="uiStore.closeContextMenu"
        class="fixed inset-0 bg-black/60 backdrop-blur-xs pointer-events-auto animate-in fade-in duration-150"
      ></div>

      <!-- Unified Menu Container (Desktop Floating Window vs Mobile Bottom Sheet) -->
      <div
        ref="menuRef"
        :style="uiStore.isMobile ? {} : computedStyle"
        :class="[
          'pointer-events-auto bg-white/95 dark:bg-[#0f1422]/95 backdrop-blur-md text-gray-700 dark:text-slate-200 select-none font-sans',
          uiStore.isMobile
            ? 'fixed inset-x-0 bottom-0 rounded-t-3xl border-t border-gray-200 dark:border-slate-800 shadow-2xl p-4 max-h-[85vh] overflow-y-auto pb-safe'
            : 'fixed border border-gray-200/90 dark:border-slate-700/80 shadow-2xl rounded-2xl py-1.5 w-64 text-xs'
        ]"
        @click.stop
      >
        <!-- Mobile Drag Indicator -->
        <div v-if="uiStore.isMobile" class="w-12 h-1.5 bg-gray-300 dark:bg-slate-700 rounded-full mx-auto mb-3"></div>

        <!-- 1. UNIFIED CONTEXT HEADER (Mobile & Multi-selection Desktop) -->
        <div
          v-if="uiStore.isMobile"
          class="flex items-center space-x-3 pb-3 mb-2 border-b border-gray-100 dark:border-slate-800"
        >
          <div class="w-9 h-9 rounded-xl bg-blue-50 dark:bg-blue-950/60 flex items-center justify-center shrink-0">
            <FbIcon
              :name="headerMeta.icon"
              size="1.4em"
              :class="headerMeta.iconClass"
            />
          </div>
          <div class="truncate flex-1">
            <p class="font-bold text-sm text-gray-900 dark:text-white truncate">
              {{ headerMeta.title }}
            </p>
            <p class="text-xs text-gray-400 dark:text-slate-500 font-mono truncate">
              {{ headerMeta.subtitle }}
            </p>
          </div>
          <button
            type="button"
            @click="uiStore.closeContextMenu"
            class="p-1.5 rounded-lg text-gray-400 hover:text-gray-700 dark:hover:text-white text-base cursor-pointer"
          >
            <FbIcon name="x" size="1.1em" />
          </button>
        </div>

        <!-- Desktop Multi-Selection Pill Header -->
        <div
          v-else-if="targetKind === 'multi'"
          class="px-3.5 py-1.5 mb-1 bg-blue-50/70 dark:bg-blue-950/50 border-b border-blue-100 dark:border-blue-900/50 text-[11px] font-semibold text-blue-700 dark:text-blue-300 flex items-center justify-between rounded-t-xl"
        >
          <div class="flex items-center space-x-1.5">
            <span class="w-2 h-2 rounded-full bg-blue-600 animate-pulse"></span>
            <span>{{ selectedCount }} Items Selected</span>
          </div>
          <button
            type="button"
            @click="handleDeselectAll"
            class="text-[10px] text-gray-400 hover:text-gray-700 dark:hover:text-white cursor-pointer"
            title="Deselect all (Esc)"
          >
            Clear
          </button>
        </div>

        <!-- Read-Only Storage Warning Pill -->
        <div
          v-if="!canWrite && targetKind !== 'blank'"
          class="px-3.5 py-1.5 mb-1 bg-amber-50 dark:bg-amber-950/40 border-b border-amber-200 dark:border-amber-800/50 text-[11px] font-semibold text-amber-700 dark:text-amber-400 flex items-center space-x-2 rounded-xl"
        >
          <FbIcon name="shield" size="1.1em" class="text-amber-500 shrink-0" />
          <span>Read-Only Storage</span>
        </div>

        <!-- ========================================================= -->
        <!-- 2. PRIMARY ACTIONS (Customized by Target Kind)            -->
        <!-- ========================================================= -->

        <!-- A. FIELD / AREA KOSONG (Blank Canvas Context Menu) -->
        <template v-if="targetKind === 'blank'">
          <div v-if="!canWrite" class="px-3.5 py-1.5 mb-1 bg-amber-50 dark:bg-amber-950/40 border-b border-amber-200 dark:border-amber-800/50 text-[11px] font-semibold text-amber-700 dark:text-amber-400 flex items-center space-x-2 rounded-xl">
            <FbIcon name="shield" size="1.1em" class="text-amber-500 shrink-0" />
            <span>Read-Only Storage</span>
          </div>

          <!-- Paste Here in blank area -->
          <button
            v-if="canWrite && workspaceStore.clipboard"
            type="button"
            @click="handlePaste"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center justify-between transition rounded-xl cursor-pointer text-blue-600 dark:text-blue-400 font-semibold"
          >
            <div class="flex items-center space-x-2.5">
              <FbIcon name="save" size="1.1em" class="shrink-0" />
              <span>Paste Here</span>
            </div>
            <span class="text-[10px] opacity-75 font-mono">Ctrl+V</span>
          </button>

          <div v-if="canWrite && workspaceStore.clipboard" class="my-1 border-t border-gray-100 dark:border-slate-800"></div>

          <!-- New File -->
          <button
            v-if="canWrite"
            type="button"
            @click="handleNewFile"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer"
          >
            <FbIcon name="new-file" size="1.1em" class="text-blue-500 shrink-0" />
            <span>+ New File</span>
          </button>

          <!-- New Folder -->
          <button
            v-if="canWrite"
            type="button"
            @click="handleNewFolder"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer"
          >
            <FbIcon name="new-folder" size="1.1em" class="text-amber-500 shrink-0" />
            <span>+ New Folder</span>
          </button>

          <!-- Upload Files -->
          <button
            v-if="canWrite"
            type="button"
            @click="handleUpload"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer"
          >
            <FbIcon name="upload" size="1.1em" class="text-emerald-500 shrink-0" />
            <span>Upload Files</span>
          </button>

          <div class="my-1 border-t border-gray-100 dark:border-slate-800"></div>

          <!-- Select All -->
          <button
            type="button"
            @click="handleSelectAll"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center justify-between transition rounded-xl cursor-pointer"
          >
            <div class="flex items-center space-x-2.5">
              <FbIcon name="select-all" size="1.1em" class="text-indigo-500 shrink-0" />
              <span>Select All</span>
            </div>
            <span class="text-[10px] text-gray-400 opacity-75 font-mono">Ctrl+A</span>
          </button>

          <!-- Toggle Hidden Dotfiles -->
          <button
            type="button"
            @click="handleToggleHiddenFiles"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center justify-between transition rounded-xl cursor-pointer"
          >
            <div class="flex items-center space-x-2.5">
              <FbIcon name="eye" size="1.1em" class="text-slate-400 shrink-0" />
              <span>{{ isShowingHidden ? 'Hide Dotfiles' : 'Show Dotfiles' }}</span>
            </div>
            <span class="text-[10px] text-gray-400 opacity-75 font-mono">Ctrl+H</span>
          </button>

          <!-- Toggle Grid / List View -->
          <button
            type="button"
            @click="handleToggleViewMode"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer"
          >
            <FbIcon name="grid" size="1.1em" class="text-gray-500 shrink-0" />
            <span>Toggle Grid / List View</span>
          </button>

          <!-- Refresh Panel -->
          <button
            type="button"
            @click="handleRefreshPanel"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center justify-between transition rounded-xl cursor-pointer"
          >
            <div class="flex items-center space-x-2.5">
              <FbIcon name="refresh" size="1.1em" class="text-gray-500 shrink-0" />
              <span>Refresh Panel</span>
            </div>
            <span class="text-[10px] text-gray-400 opacity-75 font-mono">F5</span>
          </button>
        </template>

        <!-- B. FOLDER ACTIONS -->
        <template v-else-if="targetKind === 'folder'">
          <button
            type="button"
            @click="handleOpen"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center justify-between transition rounded-xl cursor-pointer font-medium"
          >
            <div class="flex items-center space-x-2.5">
              <FbIcon name="folder" size="1.1em" class="text-amber-500 shrink-0" />
              <span>Open Folder</span>
            </div>
            <span class="text-[10px] opacity-75 font-mono">Enter</span>
          </button>

          <button
            v-if="workspaceStore.isDualPane"
            type="button"
            @click="handleOpenInOtherPanel"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center justify-between transition rounded-xl cursor-pointer text-blue-600 dark:text-blue-400"
          >
            <div class="flex items-center space-x-2.5">
              <FbIcon name="panel-right" size="1.1em" class="shrink-0" />
              <span>Open in Other Panel</span>
            </div>
            <span class="text-[10px] opacity-75 font-mono">Ctrl+Enter</span>
          </button>

          <button
            type="button"
            @click="handleSyncFolder"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer text-indigo-600 dark:text-indigo-400"
          >
            <FbIcon name="refresh" size="1.1em" class="shrink-0 text-indigo-500" />
            <span>Sync Folder...</span>
          </button>
        </template>

        <!-- C. VIDEO ACTIONS -->
        <template v-else-if="targetKind === 'video'">
          <button
            type="button"
            @click="handlePreview"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer text-rose-600 dark:text-rose-400 font-semibold"
          >
            <FbIcon name="play" size="1.1em" class="shrink-0 text-rose-500" />
            <span>Play Video</span>
          </button>

          <button
            type="button"
            @click="handleDownload"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer"
          >
            <FbIcon name="download" size="1.1em" class="text-blue-500 shrink-0" />
            <span>Download Video</span>
          </button>
        </template>

        <!-- D. AUDIO ACTIONS -->
        <template v-else-if="targetKind === 'audio'">
          <button
            type="button"
            @click="handlePreview"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer text-purple-600 dark:text-purple-400 font-semibold"
          >
            <FbIcon name="play" size="1.1em" class="shrink-0 text-purple-500" />
            <span>Play Audio</span>
          </button>

          <button
            type="button"
            @click="handleDownload"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer"
          >
            <FbIcon name="download" size="1.1em" class="text-blue-500 shrink-0" />
            <span>Download Audio</span>
          </button>
        </template>

        <!-- E. IMAGE ACTIONS -->
        <template v-else-if="targetKind === 'image'">
          <button
            type="button"
            @click="handlePreview"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer text-emerald-600 dark:text-emerald-400 font-semibold"
          >
            <FbIcon name="image" size="1.1em" class="shrink-0 text-emerald-500" />
            <span>View Image</span>
          </button>

          <button
            type="button"
            @click="handleDownload"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer"
          >
            <FbIcon name="download" size="1.1em" class="text-blue-500 shrink-0" />
            <span>Download Image</span>
          </button>
        </template>

        <!-- F. PDF DOCUMENT ACTIONS -->
        <template v-else-if="targetKind === 'pdf'">
          <button
            type="button"
            @click="handlePreview"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer text-red-600 dark:text-red-400 font-semibold"
          >
            <FbIcon name="pdf" size="1.1em" class="shrink-0 text-red-500" />
            <span>View PDF</span>
          </button>

          <button
            type="button"
            @click="handleDownload"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer"
          >
            <FbIcon name="download" size="1.1em" class="text-blue-500 shrink-0" />
            <span>Download PDF</span>
          </button>
        </template>

        <!-- G. CODE / TEXT / MARKDOWN / JSON ACTIONS -->
        <template v-else-if="targetKind === 'code'">
          <button
            type="button"
            @click="handleEditInEditor"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer text-blue-600 dark:text-blue-400 font-semibold"
          >
            <FbIcon name="code" size="1.1em" class="text-emerald-500 shrink-0" />
            <span>Edit in Code Editor</span>
          </button>

          <button
            type="button"
            @click="handleDownload"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer"
          >
            <FbIcon name="download" size="1.1em" class="text-blue-500 shrink-0" />
            <span>Download File</span>
          </button>
        </template>

        <!-- H. ARCHIVE ACTIONS -->
        <template v-else-if="targetKind === 'archive'">
          <button
            type="button"
            @click="handleOpenArchiveViewer"
            class="w-full text-left px-3.5 py-2 hover:bg-amber-500 hover:text-white flex items-center space-x-2.5 transition rounded-xl text-amber-600 dark:text-amber-400 font-semibold cursor-pointer"
          >
            <FbIcon name="archive" size="1.1em" class="shrink-0" />
            <span>Browse Archive Contents</span>
          </button>

          <button
            v-if="canWrite"
            type="button"
            @click="handleExtract"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl text-amber-500 cursor-pointer font-medium"
          >
            <FbIcon name="archive" size="1.1em" class="shrink-0" />
            <span>Extract Archive Here</span>
          </button>

          <button
            type="button"
            @click="handleDownload"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer"
          >
            <FbIcon name="download" size="1.1em" class="text-blue-500 shrink-0" />
            <span>Download Archive</span>
          </button>
        </template>

        <!-- I. GENERIC / BINARY FILE ACTIONS -->
        <template v-else-if="targetKind === 'binary'">
          <button
            type="button"
            @click="handleOpen"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer font-medium"
          >
            <FbIcon name="download" size="1.1em" class="text-blue-500 shrink-0" />
            <span>Download / Open</span>
          </button>
        </template>

        <!-- ========================================================= -->
        <!-- 3. DUAL-PANE CROSS TRANSFERS (For all items, not blank)    -->
        <!-- ========================================================= -->
        <template v-if="targetKind !== 'blank' && workspaceStore.isDualPane">
          <div class="my-1 border-t border-gray-100 dark:border-slate-800"></div>

          <button
            type="button"
            @click="handleCopyToOtherPane"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center justify-between transition rounded-xl cursor-pointer text-indigo-600 dark:text-indigo-400 font-medium"
          >
            <div class="flex items-center space-x-2.5">
              <FbIcon name="copy" size="1.1em" class="shrink-0" />
              <span>{{ isMultiSelected ? `Copy (${selectedCount} items) to Other Pane` : 'Copy to Other Pane' }}</span>
            </div>
            <span class="text-[10px] opacity-75 font-mono">F5</span>
          </button>

          <button
            v-if="canWrite"
            type="button"
            @click="handleMoveToOtherPane"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center justify-between transition rounded-xl cursor-pointer text-indigo-600 dark:text-indigo-400 font-medium"
          >
            <div class="flex items-center space-x-2.5">
              <FbIcon name="move" size="1.1em" class="shrink-0" />
              <span>{{ isMultiSelected ? `Move (${selectedCount} items) to Other Pane` : 'Move to Other Pane' }}</span>
            </div>
            <span class="text-[10px] opacity-75 font-mono">F6</span>
          </button>
        </template>

        <!-- ========================================================= -->
        <!-- 4. CLIPBOARD OPERATIONS (Copy, Cut, Paste inside folder)  -->
        <!-- ========================================================= -->
        <template v-if="targetKind !== 'blank'">
          <div class="my-1 border-t border-gray-100 dark:border-slate-800"></div>

          <!-- Copy -->
          <button
            type="button"
            @click="handleCopy"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center justify-between transition rounded-xl cursor-pointer"
          >
            <div class="flex items-center space-x-2.5">
              <FbIcon name="copy" size="1.1em" class="text-indigo-500 shrink-0" />
              <span>{{ isMultiSelected ? `Copy (${selectedCount} items)` : 'Copy' }}</span>
            </div>
            <span class="text-[10px] text-gray-400 opacity-75 font-mono">Ctrl+C</span>
          </button>

          <!-- Cut -->
          <button
            v-if="canWrite"
            type="button"
            @click="handleCut"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center justify-between transition rounded-xl cursor-pointer"
          >
            <div class="flex items-center space-x-2.5">
              <FbIcon name="move" size="1.1em" class="text-rose-500 shrink-0" />
              <span>{{ isMultiSelected ? `Cut (${selectedCount} items)` : 'Cut' }}</span>
            </div>
            <span class="text-[10px] text-gray-400 opacity-75 font-mono">Ctrl+X</span>
          </button>

          <!-- Paste inside folder (when right-clicking a folder) -->
          <button
            v-if="canWrite && targetKind === 'folder' && workspaceStore.clipboard"
            type="button"
            @click="handlePaste"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center justify-between transition rounded-xl cursor-pointer text-blue-600 dark:text-blue-400 font-semibold"
          >
            <div class="flex items-center space-x-2.5">
              <FbIcon name="save" size="1.1em" class="shrink-0" />
              <span>Paste Inside Folder</span>
            </div>
            <span class="text-[10px] opacity-75 font-mono">Ctrl+V</span>
          </button>
        </template>

        <!-- ========================================================= -->
        <!-- 5. FILE MANAGEMENT & METADATA (Compress, Rename, Star)    -->
        <!-- ========================================================= -->
        <template v-if="targetKind !== 'blank'">
          <div class="my-1 border-t border-gray-100 dark:border-slate-800"></div>

          <!-- Compress -->
          <button
            v-if="canWrite"
            type="button"
            @click="handleCompress"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer"
          >
            <FbIcon name="archive" size="1.1em" class="text-cyan-500 shrink-0" />
            <span>{{ isMultiSelected ? `Compress (${selectedCount} items)...` : 'Compress...' }}</span>
          </button>

          <!-- Rename (Single item only) -->
          <button
            v-if="canWrite && !isMultiSelected"
            type="button"
            @click="handleRename"
            class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center justify-between transition rounded-xl cursor-pointer"
          >
            <div class="flex items-center space-x-2.5">
              <FbIcon name="rename" size="1.1em" class="text-purple-500 shrink-0" />
              <span>Rename</span>
            </div>
            <span class="text-[10px] text-gray-400 opacity-75 font-mono">F2</span>
          </button>

          <!-- Star, Share, Properties (Single item only) -->
          <template v-if="!isMultiSelected">
            <!-- Toggle Star -->
            <button
              type="button"
              @click="handleToggleStar"
              class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer"
            >
              <FbIcon name="star" size="1.1em" :class="isItemStarred ? 'text-yellow-500' : 'text-gray-400'" class="shrink-0" />
              <span>{{ isItemStarred ? 'Remove from Starred' : 'Add to Starred' }}</span>
            </button>

            <!-- Share Link -->
            <button
              type="button"
              @click="handleShare"
              class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer text-blue-600 dark:text-blue-400"
            >
              <FbIcon name="share" size="1.1em" class="text-blue-500 shrink-0" />
              <span>Share Link...</span>
            </button>

            <!-- Properties / Permissions -->
            <button
              type="button"
              @click="handleProperties"
              class="w-full text-left px-3.5 py-2 hover:bg-blue-600 hover:text-white flex items-center space-x-2.5 transition rounded-xl cursor-pointer"
            >
              <FbIcon name="info" size="1.1em" class="text-cyan-500 shrink-0" />
              <span>Properties / Permissions</span>
            </button>
          </template>
        </template>

        <!-- ========================================================= -->
        <!-- 6. DESTRUCTIVE ACTIONS (Move to Trash & Shift+Del)        -->
        <!-- ========================================================= -->
        <template v-if="targetKind !== 'blank' && canWrite">
          <div class="my-1 border-t border-gray-100 dark:border-slate-800"></div>

          <!-- Move to Trash (Soft Delete) -->
          <button
            type="button"
            @click="handleDelete(false)"
            class="w-full text-left px-3.5 py-2 hover:bg-red-600 hover:text-white text-red-500 dark:text-red-400 flex items-center justify-between transition rounded-xl cursor-pointer"
          >
            <div class="flex items-center space-x-2.5">
              <FbIcon name="delete" size="1.1em" class="shrink-0" />
              <span>{{ isMultiSelected ? `Move to Trash (${selectedCount} items)` : 'Move to Trash' }}</span>
            </div>
            <span class="text-[10px] opacity-75 font-mono">Del</span>
          </button>

          <!-- Delete Permanently (Shift+Del) -->
          <button
            type="button"
            @click="handleDelete(true)"
            class="w-full text-left px-3.5 py-2 hover:bg-red-700 hover:text-white text-rose-600 dark:text-rose-400 font-semibold flex items-center justify-between transition rounded-xl cursor-pointer"
          >
            <div class="flex items-center space-x-2.5">
              <FbIcon name="trash" size="1.1em" class="shrink-0" />
              <span>{{ isMultiSelected ? `Delete Permanently (${selectedCount} items)` : 'Delete Permanently' }}</span>
            </div>
            <span class="text-[10px] opacity-75 font-mono">Shift+Del</span>
          </button>
        </template>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onUnmounted } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useConnectionStore } from '../../stores/connectionStore';
import { useStarredStore } from '../../stores/starredStore';
import { useUiStore } from '../../stores/uiStore';
import { useOverlayStore } from '../../overlays/overlayStore';
import { getDownloadUrl, readFileApi } from '../../api/files';
import { extractArchiveApi } from '../../api/archive';
import { normalizeApiError } from '../../utils/errorNormalizer';
import { formatBytes } from '../../utils/formatters';
import { PreviewResolver } from '../../services/previewResolver';
import type { IconName } from '../../utils/icons';

export type TargetKind =
  | 'blank'
  | 'multi'
  | 'folder'
  | 'video'
  | 'audio'
  | 'image'
  | 'pdf'
  | 'code'
  | 'archive'
  | 'binary';

const emit = defineEmits<{
  (e: 'openArchiveDialog', paths: string[]): void;
  (e: 'openCreateShareDialog', payload: { connectionId: string; path: string }): void;
  (e: 'openPropertiesDialog', payload: { connectionId: string; path: string }): void;
  (e: 'openArchiveViewer', payload: { connectionId: string; path: string }): void;
}>();

const workspaceStore = useWorkspaceStore();
const connStore = useConnectionStore();
const starredStore = useStarredStore();
const uiStore = useUiStore();
const overlayStore = useOverlayStore();

const menuRef = ref<HTMLElement | null>(null);
const posTop = ref(0);
const posLeft = ref(0);

const menuTransitionName = computed(() => (uiStore.isMobile ? 'ios-bottom-sheet' : 'ios-context-menu'));

const activePanelId = computed(() => uiStore.contextMenu.panelId || workspaceStore.activePanelId);

const activeConnectionId = computed(() => {
  return uiStore.contextMenu.connectionId || workspaceStore.activePanel.location.connectionId;
});

const canWrite = computed(() => {
  return connStore.canWrite(activeConnectionId.value);
});

// Multi-selection tracking
const selectedPaths = computed<string[]>(() => {
  const p = workspaceStore.getPanel(activePanelId.value);
  return p?.selectedEntries || [];
});

const isMultiSelected = computed(() => selectedPaths.value.length > 1);
const selectedCount = computed(() => selectedPaths.value.length);

const isShowingHidden = computed(() => {
  const p = workspaceStore.getPanel(activePanelId.value);
  return !!p?.view?.showHidden;
});

const isItemStarred = computed(() => {
  if (!uiStore.contextMenu.item) return false;
  return starredStore.isStarred(activeConnectionId.value, uiStore.contextMenu.item.path);
});

// Target kind determination
const targetKind = computed<TargetKind>(() => {
  if (!uiStore.contextMenu.item) return 'blank';
  if (isMultiSelected.value) return 'multi';
  if (uiStore.contextMenu.item.kind === 'directory') return 'folder';

  const raw = PreviewResolver.getKind(uiStore.contextMenu.item);
  if (raw === 'markdown' || raw === 'json') return 'code';
  return raw;
});

// Unified Header Presentation Meta
const headerMeta = computed<{ icon: IconName; iconClass: string; title: string; subtitle: string }>(() => {
  const kind = targetKind.value;
  const item = uiStore.contextMenu.item;

  if (kind === 'blank') {
    const p = workspaceStore.getPanel(activePanelId.value);
    const path = p?.location.path || '/';
    const folderName = path === '/' ? 'Root Directory' : path.split('/').filter(Boolean).pop() || 'Directory';
    return {
      icon: 'folder',
      iconClass: 'text-amber-500',
      title: folderName,
      subtitle: `${path} • Empty Canvas`,
    };
  }

  if (kind === 'multi') {
    return {
      icon: 'select-all',
      iconClass: 'text-blue-500',
      title: `${selectedCount.value} Items Selected`,
      subtitle: `${selectedCount.value} files and folders selected`,
    };
  }

  if (!item) {
    return {
      icon: 'file',
      iconClass: 'text-blue-500',
      title: 'Item',
      subtitle: '',
    };
  }

  switch (kind) {
    case 'folder':
      return {
        icon: 'folder',
        iconClass: 'text-amber-500',
        title: item.name,
        subtitle: `Folder • ${item.path}`,
      };
    case 'video':
      return {
        icon: 'video',
        iconClass: 'text-rose-500',
        title: item.name,
        subtitle: `Video • ${formatBytes(item.size || 0)}`,
      };
    case 'audio':
      return {
        icon: 'audio',
        iconClass: 'text-purple-500',
        title: item.name,
        subtitle: `Audio • ${formatBytes(item.size || 0)}`,
      };
    case 'image':
      return {
        icon: 'image',
        iconClass: 'text-emerald-500',
        title: item.name,
        subtitle: `Image • ${formatBytes(item.size || 0)}`,
      };
    case 'pdf':
      return {
        icon: 'pdf',
        iconClass: 'text-red-500',
        title: item.name,
        subtitle: `PDF • ${formatBytes(item.size || 0)}`,
      };
    case 'code':
      return {
        icon: 'code',
        iconClass: 'text-blue-500',
        title: item.name,
        subtitle: `Document / Code • ${formatBytes(item.size || 0)}`,
      };
    case 'archive':
      return {
        icon: 'archive',
        iconClass: 'text-amber-600',
        title: item.name,
        subtitle: `Archive • ${formatBytes(item.size || 0)}`,
      };
    default:
      return {
        icon: 'file',
        iconClass: 'text-slate-400',
        title: item.name,
        subtitle: `File • ${formatBytes(item.size || 0)}`,
      };
  }
});

// Viewport Clamping & Positioning
watch(
  () => [uiStore.contextMenu.visible, uiStore.contextMenu.x, uiStore.contextMenu.y],
  async ([visible]) => {
    if (visible && !uiStore.isMobile) {
      const viewportWidth = window.innerWidth;
      const viewportHeight = window.innerHeight;

      // Pre-clamp estimated dimensions
      let x = Math.min(uiStore.contextMenu.x, Math.max(8, viewportWidth - 264));
      let y = Math.min(uiStore.contextMenu.y, Math.max(8, viewportHeight - 400));
      posTop.value = Math.max(8, y);
      posLeft.value = Math.max(8, x);

      await nextTick();
      if (!menuRef.value) return;

      const menuWidth = menuRef.value.offsetWidth || 256;
      const menuHeight = menuRef.value.offsetHeight || 380;

      x = uiStore.contextMenu.x;
      y = uiStore.contextMenu.y;

      // Adjust horizontal overflow
      if (x + menuWidth > viewportWidth - 8) {
        x = Math.max(8, viewportWidth - menuWidth - 8);
      }

      // Adjust vertical overflow
      if (y + menuHeight > viewportHeight - 8) {
        y = Math.max(8, viewportHeight - menuHeight - 8);
      }

      posTop.value = y;
      posLeft.value = x;
    }
  },
  { immediate: true }
);

const computedStyle = computed(() => {
  const top = posTop.value > 0 ? posTop.value : uiStore.contextMenu.y;
  const left = posLeft.value > 0 ? posLeft.value : uiStore.contextMenu.x;
  return {
    top: `${top}px`,
    left: `${left}px`,
  };
});

// ==========================================
// Comprehensive Dismissal Listeners
// ==========================================
function handleGlobalPointerDown(e: PointerEvent) {
  if (!uiStore.contextMenu.visible) return;
  if (menuRef.value && menuRef.value.contains(e.target as Node)) {
    return;
  }
  // Clicked outside with left-click: dismiss
  if (e.button === 0) {
    uiStore.closeContextMenu();
  }
  // If right-click outside, let it propagate to re-target
}

function handleGlobalKeyDown(e: KeyboardEvent) {
  if (!uiStore.contextMenu.visible) return;
  if (e.key === 'Escape') {
    e.preventDefault();
    e.stopPropagation();
    uiStore.closeContextMenu();
  }
}

function handleGlobalScroll(e: Event) {
  if (!uiStore.contextMenu.visible) return;
  if (menuRef.value && menuRef.value.contains(e.target as Node)) {
    return;
  }
  uiStore.closeContextMenu();
}

function handleGlobalResize() {
  if (uiStore.contextMenu.visible) {
    uiStore.closeContextMenu();
  }
}

function handleGlobalBlur() {
  if (uiStore.contextMenu.visible) {
    uiStore.closeContextMenu();
  }
}

watch(
  () => uiStore.contextMenu.visible,
  (visible) => {
    if (visible) {
      window.addEventListener('pointerdown', handleGlobalPointerDown, true);
      window.addEventListener('keydown', handleGlobalKeyDown, true);
      window.addEventListener('scroll', handleGlobalScroll, true);
      window.addEventListener('resize', handleGlobalResize);
      window.addEventListener('blur', handleGlobalBlur);
    } else {
      window.removeEventListener('pointerdown', handleGlobalPointerDown, true);
      window.removeEventListener('keydown', handleGlobalKeyDown, true);
      window.removeEventListener('scroll', handleGlobalScroll, true);
      window.removeEventListener('resize', handleGlobalResize);
      window.removeEventListener('blur', handleGlobalBlur);
    }
  },
  { immediate: true }
);

onUnmounted(() => {
  window.removeEventListener('pointerdown', handleGlobalPointerDown, true);
  window.removeEventListener('keydown', handleGlobalKeyDown, true);
  window.removeEventListener('scroll', handleGlobalScroll, true);
  window.removeEventListener('resize', handleGlobalResize);
  window.removeEventListener('blur', handleGlobalBlur);
});

// ==========================================
// Unified Action Handlers
// ==========================================
function handleDeselectAll() {
  const p = workspaceStore.getPanel(activePanelId.value);
  p.selectedEntries = [];
  uiStore.closeContextMenu();
}

// 1. Play / View / Preview handler (Video, Audio, Image, PDF)
function handlePreview() {
  const item = uiStore.contextMenu.item;
  if (!item) return;
  const connId = activeConnectionId.value;
  uiStore.closeContextMenu();

  const resolution = PreviewResolver.resolve(
    item,
    connId,
    [],
    (payload) => {
      emit('openArchiveViewer', payload);
    }
  );
  void resolution.open();
}

// 2. Download handler
function handleDownload() {
  const item = uiStore.contextMenu.item;
  if (!item) return;
  const url = getDownloadUrl(activeConnectionId.value, item.path);
  window.open(url, '_blank');
  uiStore.closeContextMenu();
}

// 3. Edit in Code Editor handler
async function handleEditInEditor() {
  const item = uiStore.contextMenu.item;
  if (!item) return;
  const connId = activeConnectionId.value;
  uiStore.closeContextMenu();

  try {
    const resp = await readFileApi(connId, item.path);
    uiStore.openEditor(item, resp.content, resp.etag, connId);
  } catch (err: unknown) {
    const norm = normalizeApiError(err);
    uiStore.showToast(norm.message || 'Failed to open file in editor', 'error');
  }
}

// 4. Archive actions
function handleOpenArchiveViewer() {
  const item = uiStore.contextMenu.item;
  if (!item) return;
  const connId = activeConnectionId.value;
  uiStore.closeContextMenu();
  emit('openArchiveViewer', {
    connectionId: connId,
    path: item.path,
  });
}

async function handleExtract() {
  const item = uiStore.contextMenu.item;
  if (!item) return;
  const p = workspaceStore.getPanel(activePanelId.value);
  const connId = activeConnectionId.value;

  uiStore.closeContextMenu();
  try {
    await extractArchiveApi(connId, item.path, p.location.path);
    uiStore.showToast(`Extracted ${item.name}`, 'success');
    await workspaceStore.refresh(activePanelId.value);
  } catch (err: unknown) {
    const norm = normalizeApiError(err);
    uiStore.showToast(norm.message || 'Failed to extract archive', 'error');
  }
}

// 5. Open folder / navigation
function handleOpen() {
  const item = uiStore.contextMenu.item;
  if (!item) return;

  if (item.kind === 'directory') {
    workspaceStore.navigateTo(activePanelId.value, item.path);
  } else {
    handlePreview();
  }
  uiStore.closeContextMenu();
}

function handleOpenInOtherPanel() {
  const item = uiStore.contextMenu.item;
  if (!item || item.kind !== 'directory') return;
  workspaceStore.openInOtherPanel(activePanelId.value, item.path);
  uiStore.closeContextMenu();
}

function handleSyncFolder() {
  const item = uiStore.contextMenu.item;
  if (!item || item.kind !== 'directory') return;
  const panel = workspaceStore.getPanel(activePanelId.value);
  uiStore.openSync(panel.location.connectionId || 'local', item.path);
  uiStore.closeContextMenu();
}

// 6. Dual-pane cross transfers
async function handleCopyToOtherPane() {
  const item = uiStore.contextMenu.item;
  if (!item) return;
  const sourcePanelId = activePanelId.value;
  const destPanelId = sourcePanelId === 'left' ? 'right' : 'left';
  const p = workspaceStore.getPanel(sourcePanelId);
  const paths = p.selectedEntries.length > 0 ? p.selectedEntries : [item.path];

  uiStore.closeContextMenu();
  await workspaceStore.transferBetweenPanels(sourcePanelId, destPanelId, paths, false);
}

async function handleMoveToOtherPane() {
  const item = uiStore.contextMenu.item;
  if (!item) return;
  const sourcePanelId = activePanelId.value;
  const destPanelId = sourcePanelId === 'left' ? 'right' : 'left';
  const p = workspaceStore.getPanel(sourcePanelId);
  const paths = p.selectedEntries.length > 0 ? p.selectedEntries : [item.path];

  uiStore.closeContextMenu();
  await workspaceStore.transferBetweenPanels(sourcePanelId, destPanelId, paths, true);
}

// 7. Clipboard operations
function handleCopy() {
  workspaceStore.copySelection(activePanelId.value);
  uiStore.showToast(isMultiSelected.value ? `Copied ${selectedCount.value} items to clipboard` : 'Copied to clipboard', 'info');
  uiStore.closeContextMenu();
}

function handleCut() {
  workspaceStore.cutSelection(activePanelId.value);
  uiStore.showToast(isMultiSelected.value ? `Cut ${selectedCount.value} items to clipboard` : 'Cut to clipboard', 'info');
  uiStore.closeContextMenu();
}

async function handlePaste() {
  uiStore.closeContextMenu();
  await workspaceStore.paste(activePanelId.value);
}

// 8. Management & Metadata
function handleCompress() {
  const item = uiStore.contextMenu.item;
  const p = workspaceStore.getPanel(activePanelId.value);
  const paths = p.selectedEntries.length > 0 ? p.selectedEntries : (item ? [item.path] : []);

  emit('openArchiveDialog', paths);
  uiStore.closeContextMenu();
}

function handleRename() {
  const item = uiStore.contextMenu.item;
  if (!item) return;
  overlayStore.open({
    type: 'rename',
    panelId: activePanelId.value,
    path: item.path,
  });
  uiStore.closeContextMenu();
}

function handleToggleStar() {
  if (uiStore.contextMenu.item) {
    starredStore.toggleStar(activeConnectionId.value, uiStore.contextMenu.item);
    uiStore.showToast(isItemStarred.value ? 'Added to Starred' : 'Removed from Starred', 'success');
  }
  uiStore.closeContextMenu();
}

async function handleShare() {
  const item = uiStore.contextMenu.item;
  if (!item) return;

  if (uiStore.isMobile && typeof navigator !== 'undefined' && 'share' in navigator) {
    uiStore.closeContextMenu();
    try {
      const downloadUrl = window.location.origin + getDownloadUrl(activeConnectionId.value, item.path);
      await navigator.share({
        title: item.name,
        text: `Sharing ${item.name} from AeroFS`,
        url: downloadUrl,
      });
      return;
    } catch {
      // User cancelled
    }
  }

  emit('openCreateShareDialog', {
    connectionId: activeConnectionId.value,
    path: item.path,
  });
  uiStore.closeContextMenu();
}

function handleProperties() {
  if (uiStore.contextMenu.item) {
    emit('openPropertiesDialog', {
      connectionId: activeConnectionId.value,
      path: uiStore.contextMenu.item.path,
    });
  }
  uiStore.closeContextMenu();
}

// 9. Destructive actions
function handleDelete(permanent: boolean = false) {
  const item = uiStore.contextMenu.item;
  const p = workspaceStore.getPanel(activePanelId.value);
  const paths = p.selectedEntries.length > 0 ? p.selectedEntries : (item ? [item.path] : []);

  overlayStore.open({
    type: 'delete',
    panelId: activePanelId.value,
    paths,
    permanent,
  });
  uiStore.closeContextMenu();
}

// 10. Blank canvas actions
function handleNewFile() {
  overlayStore.open({ type: 'create', initialType: 'file', panelId: activePanelId.value });
  uiStore.closeContextMenu();
}

function handleNewFolder() {
  overlayStore.open({ type: 'create', initialType: 'directory', panelId: activePanelId.value });
  uiStore.closeContextMenu();
}

function handleUpload() {
  overlayStore.open({ type: 'upload', panelId: activePanelId.value });
  uiStore.closeContextMenu();
}

function handleSelectAll() {
  window.dispatchEvent(new CustomEvent('pane-select-all', { detail: { panelId: activePanelId.value } }));
  uiStore.closeContextMenu();
}

function handleToggleHiddenFiles() {
  const p = workspaceStore.getPanel(activePanelId.value);
  p.view.showHidden = !p.view.showHidden;
  workspaceStore.saveState();
  void workspaceStore.refresh(activePanelId.value);
  uiStore.closeContextMenu();
}

function handleToggleViewMode() {
  const p = workspaceStore.getPanel(activePanelId.value);
  p.view.viewMode = p.view.viewMode === 'grid' ? 'list' : 'grid';
  workspaceStore.saveState();
  uiStore.closeContextMenu();
}

function handleRefreshPanel() {
  uiStore.closeContextMenu();
  void workspaceStore.refresh(activePanelId.value);
}
</script>

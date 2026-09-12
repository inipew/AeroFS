<template>
  <section
    ref="paneRootRef"
    :data-pane-id="panelId"
    tabindex="0"
    @pointerdown="pane.setActive"
    @focusin="pane.setActive"
    @contextmenu="handleBlankContextMenu"
    @dragenter="dragDrop.handleDragEnter"
    @dragover="dragDrop.handleDragOver"
    @dragleave="dragDrop.handleDragLeave"
    @drop.prevent="dragDrop.handleDrop"
    class="flex-1 flex flex-col h-full bg-white dark:bg-[#0b0f19] overflow-hidden relative select-none outline-none focus:ring-1 focus:ring-blue-500/20"
    :class="[
      workspaceStore.isDualPane
        ? (pane.isActive.value
            ? 'border-t-2 border-t-blue-600 dark:border-t-blue-500'
            : 'border-t-2 border-t-transparent opacity-90')
        : ''
    ]"
  >
    <Transition name="drag-overlay">
      <div
        v-if="dragDrop.isDragOver.value && !dragDrop.hoveredFolderDrop.value"
        class="absolute inset-2 sm:inset-3 z-30 pointer-events-none rounded-3xl border-2 border-dashed border-blue-500/80 dark:border-blue-400/80 bg-blue-500/10 dark:bg-blue-600/15 backdrop-blur-xs flex flex-col items-center justify-center dropzone-active-glow"
      >
        <div class="bg-white/95 dark:bg-[#0f1422]/95 backdrop-blur-md text-gray-800 dark:text-slate-100 border border-blue-500/30 px-5 py-3 rounded-2xl shadow-2xl flex items-center space-x-3.5 transform transition-transform duration-standard ease-spring scale-100 animate-float-gentle">
          <div class="w-10 h-10 rounded-xl bg-gradient-to-tr from-blue-600 to-indigo-500 text-white flex items-center justify-center shadow-lg shadow-blue-500/30 shrink-0">
            <FbIcon :name="dragDrop.isShiftPressed.value ? 'move' : 'upload'" size="20px" />
          </div>
          <div class="flex flex-col text-left">
            <span class="font-bold text-xs text-gray-900 dark:text-white">
              {{ dragDrop.isShiftPressed.value ? 'Drop to move items' : 'Drop files to copy into this folder' }}
            </span>
            <span class="text-[11px] font-mono text-gray-400 dark:text-slate-400 truncate max-w-[200px] sm:max-w-[280px]">
              {{ panel.location.path }}
            </span>
          </div>
        </div>
      </div>
    </Transition>

    <div
      class="h-11 sm:h-12 border-b px-3 sm:px-4 flex items-center justify-between transition-colors text-xs shrink-0 backdrop-blur-md relative z-20"
      :class="[
        pane.isActive.value
          ? 'bg-blue-50/40 dark:bg-[#0d1424]/90 border-gray-200 dark:border-slate-800 text-gray-900 dark:text-white font-medium'
          : 'bg-gray-50/60 dark:bg-[#080c16]/90 border-gray-200/80 dark:border-slate-800/80 text-gray-500 dark:text-slate-400'
      ]"
    >
      <Transition name="header-morph" mode="out-in">
        <SelectionToolbar
          v-if="selection.selectedCount.value > 0"
          :selected-count="selection.selectedCount.value"
          :selected-total-size="selection.selectedTotalSize.value"
          :single-selected="selection.selectedCount.value === 1"
          @deselect="selection.clearSelection"
          @compress="handleCompress"
          @rename="handleRename"
          @copy="workspaceStore.copySelection(panelId)"
          @cut="workspaceStore.cutSelection(panelId)"
          @delete="handleDelete"
        />
        <div v-else class="flex items-center justify-between w-full h-full min-w-0">
          <PaneNavigation
            ref="paneNavRef"
            :can-go-back="panel.historyIndex > 0"
            :can-go-forward="panel.historyIndex < panel.history.length - 1"
            :path="panel.location.path"
            :connection-name="currentConnName"
            :connection-provider="connectionProvider"
            :connection-status="connectionStatus"
            :suggestions="pathSuggestions"
            @back="pane.goBack"
            @forward="pane.goForward"
            @navigate="pane.navigate"
            @refresh="pane.refresh"
          />
          <FileToolbar
            :view-mode="panel.view.viewMode"
            :show-hidden="panel.view.showHidden"
            :sort-field="panel.view.sortField as any"
            :sort-order="panel.view.sortOrder"
            :is-refreshing="dirQuery.isFetching.value"
            :is-dual-pane="workspaceStore.isDualPane"
            @update:view-mode="pane.setViewMode"
            @update:show-hidden="pane.toggleHidden"
            @update:sort-field="pane.setSort"
            @update:sort-order="(order) => pane.setSort(panel.view.sortField as any, order)"
            @new-file="overlayStore.open({ type: 'create', initialType: 'file', panelId })"
            @new-folder="overlayStore.open({ type: 'create', initialType: 'directory', panelId })"
            @upload="overlayStore.open({ type: 'upload', panelId })"
            @sync="overlayStore.open({ type: 'sync', sourceConnection: panel.location.connectionId, sourcePath: panel.location.path })"
            @select-all="selection.selectAll"
            @edit-path="paneNavRef?.openAddressBar()"
            @copy-path="copyCurrentPath"
            @swap-panels="workspaceStore.swapPanels"
            @refresh="pane.refresh"
          />
        </div>
      </Transition>
    </div>

    <div
      v-if="connectionStatus === 'orphaned'"
      class="mx-3 sm:mx-4 mt-2 px-3 sm:px-4 py-2.5 sm:py-3 rounded-2xl bg-red-500/10 border border-red-500/30 text-red-800 dark:text-red-300 flex items-center justify-between text-xs shrink-0 animate-in fade-in"
    >
      <div class="flex items-center space-x-2.5 truncate mr-2">
        <FbIcon name="info" size="18px" class="text-red-500 shrink-0" />
        <div class="truncate">
          <span class="font-bold">Storage connection unavailable</span>
          <span class="ml-1 text-[11px] opacity-80 truncate hidden sm:inline">This connection may have been disconnected or removed.</span>
        </div>
      </div>
      <div class="flex items-center space-x-1.5 shrink-0">
        <button type="button" @click.stop="workspaceStore.switchPanelConnection(panelId, 'local', '/')" class="px-2.5 py-1 bg-gray-200 dark:bg-slate-800 hover:bg-gray-300 dark:hover:bg-slate-700 text-gray-800 dark:text-slate-200 font-bold rounded-xl text-[10px] shrink-0 cursor-pointer shadow-xs transition">Switch to Local</button>
        <button v-if="workspaceStore.isDualPane" type="button" @click.stop="workspaceStore.closePanel(panelId)" class="px-2.5 py-1 bg-red-500 hover:bg-red-600 text-white font-bold rounded-xl text-[10px] shrink-0 cursor-pointer shadow-xs transition">Close</button>
      </div>
    </div>

    <div
      ref="panelContentRef"
      class="flex-1 min-h-0 overflow-y-auto overflow-x-hidden relative flex flex-col focus:outline-none"
      @click="handleContainerClick"
      @contextmenu.prevent="handleBlankContextMenu"
      @touchstart="handleTouchStart"
      @touchmove="handleTouchMove"
      @touchend="handleTouchEnd"
      @touchcancel="handleTouchEnd"
    >
      <div v-if="pullToRefresh.pullDistance.value > 0 || pullToRefresh.isRefreshing.value" class="flex items-center justify-center py-2 transition-transform select-none" :style="{ height: `${pullToRefresh.pullDistance.value}px` }">
        <div class="flex items-center space-x-2 text-blue-600 dark:text-blue-400 text-xs font-semibold">
          <FbIcon name="refresh" size="14px" :class="{ 'animate-spin': pullToRefresh.isRefreshing.value }" />
          <span>{{ pullToRefresh.isRefreshing.value ? 'Refreshing...' : 'Pull down to refresh' }}</span>
        </div>
      </div>

      <Transition :name="navTransitionName" :mode="uiStore.isMobile ? undefined : 'out-in'">
        <div :key="panel.location.path + '-' + panel.view.viewMode" class="w-full flex-1 flex flex-col min-h-0">
          <div v-if="dirQuery.isLoading.value" class="flex-1 flex flex-col items-center justify-center p-8 space-y-3">
            <div class="w-8 h-8 rounded-full border-2 border-blue-600 border-t-transparent animate-spin"></div>
            <span class="text-xs text-gray-500 dark:text-slate-400">Loading directory...</span>
          </div>
          <FileEmptyView v-else-if="displayedEntries.length === 0" :is-drag-over="dragDrop.isDragOver.value" @upload="overlayStore.open({ type: 'upload', panelId })" @new-folder="overlayStore.open({ type: 'create', initialType: 'directory', panelId })" />
          <div v-else-if="panel.view.viewMode === 'grid'" class="p-3 sm:p-4 flex-1 min-h-0">
            <FileGridView
              ref="gridViewRef"
              :entries="displayedEntries"
              :current-path="panel.location.path"
              :connection-id="panel.location.connectionId"
              :selected-paths="panel.selection.paths"
              :cut-paths="workspaceStore.clipboard?.operation === 'cut' ? workspaceStore.clipboard.paths : []"
              :dragged-paths="dragDrop.draggedPaths.value"
              :hovered-folder-drop="dragDrop.hoveredFolderDrop.value"
              :columns="gridCols"
              :scroll-element="panelContentRef"
              :has-more="dirQuery.hasMore.value"
              :is-fetching-next-page="dirQuery.isFetchingNextPage.value"
              :total-count="dirQuery.totalCount.value"
              @select="handleEntrySelect"
              @activate="activation.activateEntry"
              @navigate-up="pane.navigateUp"
              @contextmenu="handleEntryContextMenu"
              @dragstart="dragDrop.handleDragStart"
              @folder-dragover="dragDrop.handleFolderDragOver"
              @folder-dragleave="dragDrop.handleFolderDragLeave"
              @drop="dragDrop.handleDrop"
              @load-more="dirQuery.loadMore"
            />
          </div>
          <div v-else class="flex-1 min-h-0">
            <FileListView
              ref="listViewRef"
              :entries="displayedEntries"
              :current-path="panel.location.path"
              :selected-paths="panel.selection.paths"
              :cut-paths="workspaceStore.clipboard?.operation === 'cut' ? workspaceStore.clipboard.paths : []"
              :dragged-paths="dragDrop.draggedPaths.value"
              :hovered-folder-drop="dragDrop.hoveredFolderDrop.value"
              :density="uiStore.listDensity"
              :sort-field="panel.view.sortField"
              :sort-order="panel.view.sortOrder"
              :is-all-selected="selection.isAllSelected.value"
              :scroll-element="panelContentRef"
              :has-more="dirQuery.hasMore.value"
              :is-fetching-next-page="dirQuery.isFetchingNextPage.value"
              :total-count="dirQuery.totalCount.value"
              @select="handleEntrySelect"
              @activate="activation.activateEntry"
              @toggle-select="selection.toggleItem"
              @toggle-select-all="selection.selectAll"
              @navigate-up="pane.navigateUp"
              @sort="(f) => pane.setSort(f as any)"
              @contextmenu="handleEntryContextMenu"
              @dragstart="dragDrop.handleDragStart"
              @folder-dragover="dragDrop.handleFolderDragOver"
              @folder-dragleave="dragDrop.handleFolderDragLeave"
              @drop="dragDrop.handleDrop"
              @load-more="dirQuery.loadMore"
            />
          </div>
        </div>
      </Transition>
    </div>

    <FilePanelStatusBar
      :displayed-count="displayedEntries.length"
      :total-count="dirQuery.totalCount.value"
      :selected-count="selection.selectedCount.value"
      :selected-size="selection.selectedTotalSize.value"
      :total-folder-size="totalFolderSize"
      :view-mode="panel.view.viewMode"
      :stale="dirQuery.query.isStale.value"
      :error="!!dirQuery.error.value"
      :current-conn-name="currentConnName"
      :is-read-only="false"
    />
  </section>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onUnmounted } from 'vue';
import type { PanelId } from '../../types/workspace';
import type { FileEntry } from '../../types/vfs';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useConnectionStore } from '../../stores/connectionStore';
import { useUiStore } from '../../stores/uiStore';
import { useOverlayStore } from '../../overlays/overlayStore';
import { useDirectoryQuery } from '../../composables/useDirectoryQuery';
import PaneNavigation from './navigation/PaneNavigation.vue';
import FileToolbar from './toolbar/FileToolbar.vue';
import SelectionToolbar from './toolbar/SelectionToolbar.vue';
import FileListView from './views/FileListView.vue';
import FileGridView from './views/FileGridView.vue';
import FileEmptyView from './views/FileEmptyView.vue';
import FilePanelStatusBar from '../../components/browser/FilePanelStatusBar.vue';
import FbIcon from '../../components/common/FbIcon.vue';
import { usePane } from './composables/usePane';
import { usePaneShortcuts } from './composables/usePaneShortcuts';
import { useFileSelection } from './composables/useFileSelection';
import { useEntryActivation } from './composables/useEntryActivation';
import { useFileKeyboardNavigation } from './composables/useFileKeyboardNavigation';
import { useFileDragDrop } from './composables/useFileDragDrop';
import { usePullToRefresh } from './composables/usePullToRefresh';
import { getFileExt } from '../../utils/fileTypes';
import { getNavTransitionName } from '../../motion/tokens';

const props = defineProps<{ panelId: PanelId }>();
const workspaceStore = useWorkspaceStore();
const connStore = useConnectionStore();
const uiStore = useUiStore();
const overlayStore = useOverlayStore();
const pane = usePane(props.panelId);
const panel = pane.panel;
const paneRootRef = ref<HTMLElement | null>(null);
const panelContentRef = ref<HTMLElement | null>(null);
const paneNavRef = ref<InstanceType<typeof PaneNavigation> | null>(null);
const listViewRef = ref<InstanceType<typeof FileListView> | null>(null);
const gridViewRef = ref<InstanceType<typeof FileGridView> | null>(null);
const containerWidth = ref(0);
const connectionIdRef = computed(() => panel.value.location.connectionId);
const pathRef = computed(() => panel.value.location.path);
const queryParamsRef = computed(() => ({ show_hidden: panel.value.view.showHidden, sort: panel.value.view.sortField, order: panel.value.view.sortOrder, limit: 100 }));
const dirQuery = useDirectoryQuery(connectionIdRef, pathRef, queryParamsRef);

watch(() => dirQuery.entries.value, (newEntries) => {
  const validPaths = new Set((newEntries || []).map((e) => e.path));
  panel.value.selectedEntries = panel.value.selectedEntries.filter((p) => validPaths.has(p));
}, { immediate: true, deep: true });

const currentConn = computed(() => connStore.connections.find((c) => c.id === panel.value.location.connectionId));
const currentConnName = computed(() => currentConn.value?.name || (panel.value.location.connectionId === 'local' ? 'Local' : panel.value.location.connectionId));
const connectionProvider = computed(() => currentConn.value?.provider || 'local');
const connectionStatus = computed(() => (currentConn.value as any)?.status || 'connected');

const displayedEntries = computed<FileEntry[]>(() => {
  const list = [...(dirQuery.entries.value || [])];
  const field = panel.value.view.sortField || 'name';
  const order = panel.value.view.sortOrder === 'desc' ? -1 : 1;
  list.sort((a, b) => {
    if (a.kind === 'directory' && b.kind !== 'directory') return -1;
    if (a.kind !== 'directory' && b.kind === 'directory') return 1;
    if (field === 'size') return ((a.size || 0) - (b.size || 0)) * order;
    if (field === 'modified') {
      const dateA = a.modified_at ? new Date(a.modified_at).getTime() : 0;
      const dateB = b.modified_at ? new Date(b.modified_at).getTime() : 0;
      return (dateA - dateB) * order;
    }
    if (field === 'type') return getFileExt(a).localeCompare(getFileExt(b)) * order;
    return a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: 'base' }) * order;
  });
  return list;
});

const displayedFiles = computed(() => displayedEntries.value.filter((e) => e.kind !== 'directory'));
const totalFolderSize = computed(() => displayedEntries.value.reduce((acc, curr) => acc + (curr.size || 0), 0));
const pathSuggestions = computed(() => displayedEntries.value.filter((e) => e.kind === 'directory').map((e) => e.path));
const gridCols = computed(() => {
  const w = containerWidth.value;
  if (w >= 1280) return 8;
  if (w >= 1024) return 6;
  if (w >= 768) return 4;
  if (w >= 640) return 3;
  return 2;
});

let resizeObserver: ResizeObserver | null = null;
onMounted(() => {
  if (panelContentRef.value) {
    containerWidth.value = panelContentRef.value.offsetWidth;
    resizeObserver = new ResizeObserver((entries) => { containerWidth.value = entries[0]?.contentRect.width ?? 0; });
    resizeObserver.observe(panelContentRef.value);
  }
});
onUnmounted(() => resizeObserver?.disconnect());
const selectedPathsRef = computed(() => panel.value.selectedEntries);
const selection = useFileSelection({ entries: displayedEntries, selectedPaths: selectedPathsRef, onSelect: (paths) => { panel.value.selectedEntries = paths; } });
const activation = useEntryActivation({ connectionId: computed(() => panel.value.location.connectionId), displayedFiles, onNavigate: pane.navigate });
const dragDrop = useFileDragDrop({ panelId: props.panelId, connectionId: computed(() => panel.value.location.connectionId), currentPath: computed(() => panel.value.location.path), entries: displayedEntries, selectedPaths: selectedPathsRef, containerRef: paneRootRef, onSelect: (paths) => { panel.value.selectedEntries = paths; } });
const navTransitionName = computed(() => getNavTransitionName(panel.value.navigationDirection || 'replace'));

usePaneShortcuts({
  isActive: pane.isActive,
  isDualPane: computed(() => workspaceStore.isDualPane),
  onBack: pane.goBack,
  onForward: pane.goForward,
  onNavigateUp: pane.navigateUp,
  onRefresh: pane.refresh,
  onOpenAddressBar: () => paneNavRef.value?.openAddressBar(),
  onSwapPanels: () => workspaceStore.swapPanels(),
  onSwitchActivePanel: (target) => target ? workspaceStore.setActivePanel(target) : workspaceStore.setActivePanel(props.panelId === 'left' ? 'right' : 'left'),
  onCopy: () => { workspaceStore.copySelection(props.panelId); uiStore.showToast('Copied to clipboard', 'info'); },
  onCut: () => { workspaceStore.cutSelection(props.panelId); uiStore.showToast('Cut to clipboard', 'info'); },
  onPaste: () => { void workspaceStore.paste(props.panelId); },
  onOpenInOtherPanel: () => {
    if (panel.value.selectedEntries.length === 1) {
      const selected = displayedEntries.value.find((e) => e.path === panel.value.selectedEntries[0]);
      if (selected?.kind === 'directory') workspaceStore.openInOtherPanel(props.panelId, selected.path);
    }
  },
});

useFileKeyboardNavigation({
  isActive: pane.isActive,
  viewMode: computed(() => panel.value.view.viewMode),
  displayedEntries,
  selectedPaths: selectedPathsRef,
  activeCursorIndex: selection.lastClickedIndex,
  gridCols,
  onSelectIndex: (idx, modifiers) => {
    selection.selectIndex(idx, modifiers);
    const target = displayedEntries.value[idx];
    if (target) {
      if (panel.value.view.viewMode === 'grid') gridViewRef.value?.scrollToIndex(idx); else listViewRef.value?.scrollToIndex(idx);
      nextTick(() => {
        const safePath = window.CSS?.escape ? window.CSS.escape(target.path) : target.path.replace(/"/g, '\\"');
        const el = panelContentRef.value?.querySelector(`[data-entry-path="${safePath}"]`);
        if (el) (el as HTMLElement).scrollIntoView({ block: 'nearest', behavior: 'smooth' });
      });
    }
  },
  onActivate: activation.activateEntry,
  onNavigateUp: pane.navigateUp,
  onRename: (entry) => overlayStore.open({ type: 'rename', panelId: props.panelId, path: entry.path }),
  onDelete: (paths, permanent) => overlayStore.open({ type: 'delete', panelId: props.panelId, paths, permanent }),
  onSelectAll: selection.selectAll,
  onClearSelection: selection.clearSelection,
});

const pullToRefresh = usePullToRefresh({ containerRef: panelContentRef, enabled: computed(() => uiStore.isMobile), onRefresh: pane.refresh });
function handleEntrySelect(e: MouseEvent, entry: FileEntry) { selection.handleEntrySelect(entry, { shiftKey: e.shiftKey, ctrlKey: e.ctrlKey, metaKey: e.metaKey }); }
function handleContainerClick(e: MouseEvent) {
  const target = e.target as HTMLElement;
  if (target.closest('[data-entry-item]') || target.closest('button') || target.closest('input') || target.closest('a') || target.closest('th')) return;
  pane.setActive(); selection.clearSelection();
}
let touchStartTime = 0;
let touchStartPos = { x: 0, y: 0 };
let isTwoFingerTap = false;
let longPressTimeout: ReturnType<typeof setTimeout> | null = null;
let touchTargetEntry: FileEntry | null = null;
function handleTouchStart(e: TouchEvent) {
  if (e.touches.length === 2) {
    isTwoFingerTap = true; touchStartTime = Date.now(); touchStartPos = { x: (e.touches[0].clientX + e.touches[1].clientX) / 2, y: (e.touches[0].clientY + e.touches[1].clientY) / 2 };
    const targetEl = (e.target as HTMLElement | null)?.closest('[data-entry-path]') as HTMLElement | null;
    const path = targetEl?.getAttribute('data-entry-path'); touchTargetEntry = path ? displayedEntries.value.find((x) => x.path === path) || null : null;
    if (longPressTimeout) { clearTimeout(longPressTimeout); longPressTimeout = null; }
    return;
  }
  if (e.touches.length === 1) {
    pullToRefresh.onTouchStart(e); isTwoFingerTap = false; touchStartTime = Date.now(); touchStartPos = { x: e.touches[0].clientX, y: e.touches[0].clientY };
    const targetEl = (e.target as HTMLElement | null)?.closest('[data-entry-path]') as HTMLElement | null;
    const path = targetEl?.getAttribute('data-entry-path'); touchTargetEntry = path ? displayedEntries.value.find((x) => x.path === path) || null : null;
    if (touchTargetEntry) {
      if (longPressTimeout) clearTimeout(longPressTimeout);
      longPressTimeout = setTimeout(() => {
        try { navigator.vibrate?.(35); } catch {}
        pane.setActive();
        if (touchTargetEntry && !panel.value.selectedEntries.includes(touchTargetEntry.path)) {
          panel.value.selectedEntries = [touchTargetEntry.path];
          selection.lastClickedIndex.value = displayedEntries.value.findIndex((x) => x.path === touchTargetEntry!.path);
        }
        uiStore.openContextMenu({ clientX: touchStartPos.x, clientY: touchStartPos.y, preventDefault: () => {} } as MouseEvent, touchTargetEntry as any, panel.value.location.connectionId, props.panelId);
        longPressTimeout = null;
      }, 500);
    }
  }
}
function handleTouchMove(e: TouchEvent) {
  if (e.touches.length === 1) {
    pullToRefresh.onTouchMove(e);
    if (longPressTimeout) {
      const dx = Math.abs(e.touches[0].clientX - touchStartPos.x); const dy = Math.abs(e.touches[0].clientY - touchStartPos.y);
      if (dx > 10 || dy > 10) { clearTimeout(longPressTimeout); longPressTimeout = null; }
    }
  }
}
function handleTouchEnd(e: TouchEvent) {
  pullToRefresh.onTouchEnd();
  if (longPressTimeout) { clearTimeout(longPressTimeout); longPressTimeout = null; }
  if (isTwoFingerTap) {
    const elapsed = Date.now() - touchStartTime;
    if (elapsed < 400) {
      e.preventDefault(); try { navigator.vibrate?.(30); } catch {}
      pane.setActive();
      if (touchTargetEntry) {
        if (!panel.value.selectedEntries.includes(touchTargetEntry.path)) { panel.value.selectedEntries = [touchTargetEntry.path]; selection.lastClickedIndex.value = displayedEntries.value.findIndex((x) => x.path === touchTargetEntry!.path); }
      } else selection.clearSelection();
      uiStore.openContextMenu({ clientX: touchStartPos.x, clientY: touchStartPos.y, preventDefault: () => {} } as MouseEvent, touchTargetEntry as any, panel.value.location.connectionId, props.panelId);
    }
    isTwoFingerTap = false; touchTargetEntry = null;
  }
}
function handleEntryContextMenu(e: MouseEvent, entry: FileEntry) {
  e.preventDefault(); e.stopPropagation(); pane.setActive();
  if (!entry) { handleBlankContextMenu(e); return; }
  if (!panel.value.selectedEntries.includes(entry.path)) { panel.value.selectedEntries = [entry.path]; selection.lastClickedIndex.value = displayedEntries.value.findIndex((x) => x.path === entry.path); }
  uiStore.openContextMenu(e, entry as any, panel.value.location.connectionId, props.panelId);
}
function handleBlankContextMenu(e: MouseEvent) {
  const target = e.target as HTMLElement;
  if (target?.closest('input, textarea, [contenteditable="true"]') || target?.closest('[data-entry-item]')) return;
  e.preventDefault(); e.stopPropagation(); pane.setActive(); selection.clearSelection(); uiStore.openContextMenu(e, null, panel.value.location.connectionId, props.panelId);
}
function handleCompress() { overlayStore.open({ type: 'archive', connectionId: panel.value.location.connectionId, basePath: panel.value.location.path, selectedPaths: panel.value.selectedEntries }); }
function handleRename() { if (panel.value.selectedEntries.length === 1) overlayStore.open({ type: 'rename', panelId: props.panelId, path: panel.value.selectedEntries[0] }); }
function handleDelete(permanent: boolean = false) { overlayStore.open({ type: 'delete', panelId: props.panelId, paths: panel.value.selectedEntries, permanent }); }
async function copyCurrentPath() { try { await navigator.clipboard.writeText(panel.value.location.path); uiStore.showToast(`Copied path: ${panel.value.location.path}`, 'info'); } catch { uiStore.showToast('Failed to copy path', 'error'); } }
function onSelectAllEvent(e: Event) { const custom = e as CustomEvent<{ panelId: string }>; if (custom.detail?.panelId === props.panelId) selection.selectAll(); }
onMounted(() => window.addEventListener('pane-select-all', onSelectAllEvent));
onUnmounted(() => window.removeEventListener('pane-select-all', onSelectAllEvent));
</script>

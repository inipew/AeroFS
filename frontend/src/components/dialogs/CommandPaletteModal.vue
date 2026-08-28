<template>
  <Transition name="spotlight-modal">
    <div
      v-if="uiStore.isCommandPaletteOpen"
      class="fixed inset-0 z-50 bg-black/60 backdrop-blur-xs flex items-start justify-center pt-12 sm:pt-20 px-3 select-none font-sans text-xs"
      @click="uiStore.isCommandPaletteOpen = false"
    >
      <div
        class="spotlight-card bg-white dark:bg-[#0c101c] border border-gray-200 dark:border-slate-800 rounded-3xl max-w-xl w-full shadow-2xl overflow-hidden flex flex-col max-h-[75vh]"
        @click.stop
      >
      <!-- Search Input Bar -->
      <div class="h-14 border-b border-gray-200 dark:border-slate-800 px-4 flex items-center space-x-3 bg-gray-50/50 dark:bg-[#090d18] shrink-0">
        <FbIcon name="search" size="18px" class="text-gray-400 dark:text-slate-500 shrink-0" />
        <input
          ref="searchInputRef"
          v-model="searchQuery"
          @keydown="handleKeyDown"
          type="text"
          placeholder="Type a command or action... (e.g. split, new, upload, sftp, list)"
          class="flex-1 bg-transparent text-sm text-gray-900 dark:text-white placeholder-gray-400 dark:placeholder-slate-500 outline-none font-medium"
        />
        <kbd class="hidden sm:inline-block px-2 py-0.5 rounded-md bg-gray-200 dark:bg-slate-800 text-[10px] font-mono text-gray-500 dark:text-slate-400 border border-gray-300 dark:border-slate-700">
          ESC
        </kbd>
      </div>

      <!-- Commands List -->
      <div class="overflow-y-auto p-2 space-y-1 flex-1">
        <div v-if="filteredCommands.length === 0" class="py-12 text-center text-gray-400 dark:text-slate-500">
          <p class="font-medium text-xs">No matching commands found</p>
          <p class="text-[11px] mt-1">Try searching for "split", "folder", "view", or "settings"</p>
        </div>

        <template v-else>
          <button
            v-for="(cmd, idx) in filteredCommands"
            :key="cmd.id"
            @click="executeCommand(cmd)"
            @mouseenter="selectedIndex = idx"
            :class="[
              'w-full px-3.5 py-2.5 rounded-2xl flex items-center justify-between transition cursor-pointer text-left',
              selectedIndex === idx
                ? 'bg-blue-600 text-white font-semibold shadow-xs'
                : 'text-gray-800 dark:text-slate-200 hover:bg-gray-100 dark:hover:bg-slate-800/70'
            ]"
          >
            <div class="flex items-center space-x-3 truncate">
              <span :class="['text-base shrink-0', selectedIndex === idx ? 'text-white' : 'text-blue-600 dark:text-blue-400']">
                {{ cmd.icon }}
              </span>
              <div class="truncate">
                <span class="text-xs truncate">{{ cmd.title }}</span>
                <span :class="['ml-2 text-[10px] font-normal truncate opacity-70', selectedIndex === idx ? 'text-blue-100' : 'text-gray-400 dark:text-slate-500']">
                  {{ cmd.category }}
                </span>
              </div>
            </div>

            <div v-if="cmd.shortcut" class="shrink-0 ml-3">
              <kbd :class="['px-2 py-0.5 rounded-md text-[10px] font-mono border', selectedIndex === idx ? 'bg-blue-700 border-blue-500 text-white' : 'bg-gray-100 dark:bg-slate-800 border-gray-200 dark:border-slate-700 text-gray-400 dark:text-slate-400']">
                {{ cmd.shortcut }}
              </kbd>
            </div>
          </button>
        </template>
      </div>

      <!-- Footer Bar -->
      <div class="h-8 bg-gray-50 dark:bg-[#090d18] border-t border-gray-200 dark:border-slate-800 px-4 flex items-center justify-between text-[10px] text-gray-400 dark:text-slate-500 font-mono shrink-0">
        <div class="flex items-center space-x-3">
          <span>↑↓ to navigate</span>
          <span>↵ to execute</span>
        </div>
        <span>AeroFS Command Palette</span>
      </div>
    </div>
  </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick } from 'vue';
import FbIcon from '../common/FbIcon.vue';
import { useUiStore } from '../../stores/uiStore';
import { useWorkspaceStore } from '../../stores/workspaceStore';
import { useConnectionStore } from '../../stores/connectionStore';

const emit = defineEmits<{
  (e: 'openSettings'): void;
  (e: 'openConnectionDialog'): void;
  (e: 'openSearchDialog'): void;
}>();

const uiStore = useUiStore();
const workspaceStore = useWorkspaceStore();
const connStore = useConnectionStore();

const searchInputRef = ref<HTMLInputElement | null>(null);
const searchQuery = ref('');
const selectedIndex = ref(0);

interface PaletteCommand {
  id: string;
  title: string;
  category: string;
  icon: string;
  shortcut?: string;
  action: () => void;
}

const commands = computed<PaletteCommand[]>(() => {
  const list: PaletteCommand[] = [
    // 1. Files & Folders
    {
      id: 'new-folder',
      title: 'New Folder',
      category: 'File Actions',
      icon: '📁',
      action: () => uiStore.openCreate('directory'),
    },
    {
      id: 'new-file',
      title: 'New File',
      category: 'File Actions',
      icon: '📄',
      action: () => uiStore.openCreate('file'),
    },
    {
      id: 'upload-files',
      title: 'Upload Files / Media',
      category: 'File Actions',
      icon: '⬆️',
      action: () => uiStore.openUpload(),
    },
    {
      id: 'search-files',
      title: 'Search Files in Directory',
      category: 'Navigation',
      icon: '🔍',
      shortcut: 'Ctrl+F',
      action: () => emit('openSearchDialog'),
    },

    // 2. Workspace & Layout
    {
      id: 'toggle-dual-pane',
      title: workspaceStore.isDualPane ? 'Switch to Single Pane' : 'Split View (Dual Pane)',
      category: 'Workspace Layout',
      icon: '🗂️',
      action: () => {
        workspaceStore.isDualPane = !workspaceStore.isDualPane;
      },
    },
    {
      id: 'swap-panels',
      title: 'Swap Left and Right Panels',
      category: 'Workspace Layout',
      icon: '🔄',
      action: () => workspaceStore.swapPanels(),
    },
    {
      id: 'refresh-panel',
      title: 'Refresh Active Panel',
      category: 'Navigation',
      icon: '⟳',
      action: () => workspaceStore.refreshPanel(workspaceStore.activePanelId),
    },

    // 3. View & Display
    {
      id: 'toggle-view-mode',
      title: 'Toggle Grid / List View',
      category: 'View',
      icon: '🔀',
      action: () => {
        const p = workspaceStore.getPanel(workspaceStore.activePanelId);
        p.viewMode = p.viewMode === 'grid' ? 'list' : 'grid';
        workspaceStore.saveState();
      },
    },
    {
      id: 'toggle-hidden-files',
      title: 'Toggle Hidden (Dot) Files',
      category: 'View',
      icon: '👁️',
      shortcut: 'Ctrl+H',
      action: () => {
        const p = workspaceStore.getPanel(workspaceStore.activePanelId);
        p.showHidden = !p.showHidden;
        workspaceStore.saveState();
        workspaceStore.fetchPanelEntries(workspaceStore.activePanelId);
      },
    },
    {
      id: 'density-comfortable',
      title: 'Density: Comfortable',
      category: 'Appearance',
      icon: '📏',
      action: () => uiStore.setListDensity('comfortable'),
    },
    {
      id: 'density-compact',
      title: 'Density: Compact',
      category: 'Appearance',
      icon: '📐',
      action: () => uiStore.setListDensity('compact'),
    },
    {
      id: 'density-dense',
      title: 'Density: Dense (Power-User)',
      category: 'Appearance',
      icon: '⚡',
      action: () => uiStore.setListDensity('dense'),
    },

    // 4. Storage Connections
    {
      id: 'open-connections',
      title: 'Manage Storage Connections',
      category: 'Storage',
      icon: '💾',
      action: () => emit('openConnectionDialog'),
    },
    {
      id: 'open-settings',
      title: 'Preferences & Settings',
      category: 'System',
      icon: '⚙️',
      shortcut: 'Ctrl+,',
      action: () => emit('openSettings'),
    },
  ];

  // Dynamic Connections
  connStore.connections.forEach((conn) => {
    list.push({
      id: `switch-conn-${conn.id}`,
      title: `Switch to ${conn.name} (${conn.provider.toUpperCase()})`,
      category: 'Storage Connections',
      icon: '🌐',
      action: () => {
        workspaceStore.switchPanelConnection(workspaceStore.activePanelId, conn.id);
      },
    });
  });

  // Dynamic Workspace Presets
  workspaceStore.presets.forEach((pr) => {
    list.push({
      id: `preset-${pr.id}`,
      title: `Apply Preset: ${pr.name}`,
      category: 'Workspace Presets',
      icon: '🗂️',
      action: () => {
        workspaceStore.applyPreset(pr.id);
        uiStore.showToast(`Applied preset "${pr.name}"`, 'info');
      },
    });
  });

  return list;
});

const filteredCommands = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  if (!q) return commands.value;
  return commands.value.filter(
    (c) =>
      c.title.toLowerCase().includes(q) ||
      c.category.toLowerCase().includes(q) ||
      c.id.toLowerCase().includes(q)
  );
});

watch(
  () => uiStore.isCommandPaletteOpen,
  (open) => {
    if (open) {
      searchQuery.value = '';
      selectedIndex.value = 0;
      nextTick(() => {
        searchInputRef.value?.focus();
      });
    }
  }
);

watch(
  () => filteredCommands.value,
  () => {
    selectedIndex.value = 0;
  }
);

function handleKeyDown(e: KeyboardEvent) {
  if (e.key === 'ArrowDown') {
    e.preventDefault();
    selectedIndex.value = (selectedIndex.value + 1) % filteredCommands.value.length;
  } else if (e.key === 'ArrowUp') {
    e.preventDefault();
    selectedIndex.value = (selectedIndex.value - 1 + filteredCommands.value.length) % filteredCommands.value.length;
  } else if (e.key === 'Enter') {
    e.preventDefault();
    if (filteredCommands.value[selectedIndex.value]) {
      executeCommand(filteredCommands.value[selectedIndex.value]);
    }
  } else if (e.key === 'Escape') {
    uiStore.isCommandPaletteOpen = false;
  }
}

function executeCommand(cmd: PaletteCommand) {
  uiStore.isCommandPaletteOpen = false;
  cmd.action();
}
</script>

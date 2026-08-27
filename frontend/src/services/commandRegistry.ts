import type { IconName } from '../utils/icons';
import { useWorkspaceStore } from '../stores/workspaceStore';
import { useConnectionStore } from '../stores/connectionStore';
import { useUiStore } from '../stores/uiStore';
import { useHistoryStore } from '../stores/historyStore';
import type { FileEntry } from '../types/vfs';

export interface CommandContext {
  connectionId?: string;
  panelId?: 'left' | 'right';
  currentPath?: string;
  selectedPaths?: string[];
  focusedItem?: FileEntry;
}

export interface CommandDefinition {
  id: string;
  label: string;
  category: 'File' | 'Edit' | 'View' | 'Navigation' | 'Storage' | 'System';
  shortcut?: string;
  icon?: IconName;
  description?: string;
  enabled?: (ctx: CommandContext) => boolean;
  execute: (ctx: CommandContext) => Promise<void> | void;
}

class CommandRegistry {
  private commands = new Map<string, CommandDefinition>();

  public register(cmd: CommandDefinition) {
    this.commands.set(cmd.id, cmd);
  }

  public get(id: string): CommandDefinition | undefined {
    return this.commands.get(id);
  }

  public getAll(): CommandDefinition[] {
    return Array.from(this.commands.values());
  }

  public getByCategory(category: CommandDefinition['category']): CommandDefinition[] {
    return this.getAll().filter((c) => c.category === category);
  }

  public async execute(id: string, customCtx?: Partial<CommandContext>): Promise<boolean> {
    const cmd = this.commands.get(id);
    if (!cmd) {
      console.warn(`Command '${id}' not found in CommandRegistry`);
      return false;
    }

    const workspaceStore = useWorkspaceStore();
    const activePanel = workspaceStore.getPanel(workspaceStore.activePanelId);

    const ctx: CommandContext = {
      connectionId: activePanel.connectionId,
      panelId: workspaceStore.activePanelId,
      currentPath: activePanel.path,
      selectedPaths: [...activePanel.selectedEntries],
      focusedItem: undefined,
      ...customCtx,
    };

    if (cmd.enabled && !cmd.enabled(ctx)) {
      return false;
    }

    try {
      await cmd.execute(ctx);
      return true;
    } catch (err: any) {
      console.error(`Error executing command ${id}:`, err);
      const uiStore = useUiStore();
      uiStore.showToast(err?.message || `Failed to execute ${cmd.label}`, 'error');
      return false;
    }
  }
}

export const commandRegistry = new CommandRegistry();

// Initialize all core commands
export function initializeCommandRegistry() {
  const ws = () => useWorkspaceStore();
  const conn = () => useConnectionStore();
  const ui = () => useUiStore();
  const hist = () => useHistoryStore();

  // --- FILE COMMANDS ---
  commandRegistry.register({
    id: 'file.new-file',
    label: 'New File',
    category: 'File',
    shortcut: 'N',
    icon: 'new-file',
    enabled: (ctx) => conn().canWrite(ctx.connectionId || 'local'),
    execute: () => {
      ui().openCreate('file');
    },
  });

  commandRegistry.register({
    id: 'file.new-folder',
    label: 'New Folder',
    category: 'File',
    shortcut: 'F7',
    icon: 'new-folder',
    enabled: (ctx) => conn().canWrite(ctx.connectionId || 'local'),
    execute: () => {
      ui().openCreate('directory');
    },
  });

  commandRegistry.register({
    id: 'file.upload',
    label: 'Upload Files',
    category: 'File',
    shortcut: 'Ctrl+U',
    icon: 'upload',
    enabled: (ctx) => conn().canWrite(ctx.connectionId || 'local'),
    execute: () => {
      ui().openUpload();
    },
  });

  commandRegistry.register({
    id: 'file.rename',
    label: 'Rename',
    category: 'File',
    shortcut: 'F2',
    icon: 'rename',
    enabled: (ctx) =>
      conn().canWrite(ctx.connectionId || 'local') &&
      (ctx.selectedPaths?.length === 1 || !!ctx.focusedItem),
    execute: (ctx) => {
      const item = ctx.focusedItem || ws().getPanel(ws().activePanelId).entries.find((e) => e.path === ctx.selectedPaths?.[0]);
      if (item) {
        ui().openRename(item);
      }
    },
  });

  commandRegistry.register({
    id: 'file.delete',
    label: 'Delete',
    category: 'File',
    shortcut: 'Delete',
    icon: 'delete',
    enabled: (ctx) =>
      conn().canWrite(ctx.connectionId || 'local') &&
      ((ctx.selectedPaths && ctx.selectedPaths.length > 0) || !!ctx.focusedItem),
    execute: (ctx) => {
      const paths = ctx.focusedItem ? [ctx.focusedItem.path] : ctx.selectedPaths || [];
      if (paths.length > 0) {
        ui().openDelete(paths);
      }
    },
  });

  // --- EDIT COMMANDS ---
  commandRegistry.register({
    id: 'edit.copy',
    label: 'Copy Selection',
    category: 'Edit',
    shortcut: 'Ctrl+C',
    icon: 'copy',
    enabled: (ctx) => (ctx.selectedPaths && ctx.selectedPaths.length > 0) || !!ctx.focusedItem,
    execute: (ctx) => {
      const pid = ctx.panelId || ws().activePanelId;
      ws().copySelection(pid);
    },
  });

  commandRegistry.register({
    id: 'edit.cut',
    label: 'Cut Selection',
    category: 'Edit',
    shortcut: 'Ctrl+X',
    icon: 'move',
    enabled: (ctx) =>
      conn().canWrite(ctx.connectionId || 'local') &&
      ((ctx.selectedPaths && ctx.selectedPaths.length > 0) || !!ctx.focusedItem),
    execute: (ctx) => {
      const pid = ctx.panelId || ws().activePanelId;
      ws().cutSelection(pid);
    },
  });

  commandRegistry.register({
    id: 'edit.paste',
    label: 'Paste',
    category: 'Edit',
    shortcut: 'Ctrl+V',
    icon: 'copy',
    enabled: (ctx) => conn().canWrite(ctx.connectionId || 'local') && ws().clipboard !== null,
    execute: (ctx) => {
      const pid = ctx.panelId || ws().activePanelId;
      ws().paste(pid);
    },
  });

  commandRegistry.register({
    id: 'edit.undo',
    label: 'Undo Last Operation',
    category: 'Edit',
    shortcut: 'Ctrl+Z',
    icon: 'arrow-back',
    enabled: () => hist().canUndo,
    execute: async () => {
      await hist().undo();
    },
  });

  commandRegistry.register({
    id: 'edit.redo',
    label: 'Redo Last Operation',
    category: 'Edit',
    shortcut: 'Ctrl+Y',
    icon: 'chevron-right',
    enabled: () => hist().canRedo,
    execute: async () => {
      await hist().redo();
    },
  });

  // --- VIEW & WORKSPACE COMMANDS ---
  commandRegistry.register({
    id: 'view.toggle-split',
    label: 'Toggle Split Dual-Pane',
    category: 'View',
    shortcut: 'Ctrl+\\',
    icon: 'panel-right',
    execute: () => {
      ws().setDualPane(!ws().isDualPane);
    },
  });

  commandRegistry.register({
    id: 'view.swap-panels',
    label: 'Swap Left & Right Panels',
    category: 'View',
    shortcut: 'Ctrl+Shift+S',
    icon: 'refresh',
    enabled: () => ws().isDualPane,
    execute: () => {
      ws().swapPanels();
    },
  });

  commandRegistry.register({
    id: 'view.toggle-hidden',
    label: 'Toggle Dotfiles / Hidden Files',
    category: 'View',
    shortcut: 'Ctrl+H',
    icon: 'settings',
    execute: (ctx) => {
      const pid = ctx.panelId || ws().activePanelId;
      ws().toggleShowHidden(pid);
    },
  });

  commandRegistry.register({
    id: 'view.refresh',
    label: 'Refresh Directory',
    category: 'Navigation',
    shortcut: 'F5',
    icon: 'refresh',
    execute: (ctx) => {
      const pid = ctx.panelId || ws().activePanelId;
      ws().refreshPanel(pid);
    },
  });

  commandRegistry.register({
    id: 'nav.go-back',
    label: 'Navigate Back',
    category: 'Navigation',
    shortcut: 'Alt+Left',
    icon: 'chevron-left',
    execute: (ctx) => {
      const pid = ctx.panelId || ws().activePanelId;
      ws().goBackPanel(pid);
    },
  });

  commandRegistry.register({
    id: 'nav.go-forward',
    label: 'Navigate Forward',
    category: 'Navigation',
    shortcut: 'Alt+Right',
    icon: 'chevron-right',
    execute: (ctx) => {
      const pid = ctx.panelId || ws().activePanelId;
      ws().goForwardPanel(pid);
    },
  });

  commandRegistry.register({
    id: 'nav.go-up',
    label: 'Navigate Up to Parent Directory',
    category: 'Navigation',
    shortcut: 'Alt+Up',
    icon: 'arrow-up',
    execute: (ctx) => {
      const pid = ctx.panelId || ws().activePanelId;
      ws().navigateUpPanel(pid);
    },
  });

  // --- SYSTEM & DIALOG COMMANDS ---
  commandRegistry.register({
    id: 'system.command-palette',
    label: 'Command Palette',
    category: 'System',
    shortcut: 'Ctrl+K',
    icon: 'search',
    execute: () => {
      ui().toggleCommandPalette();
    },
  });

  commandRegistry.register({
    id: 'system.search',
    label: 'Search Files',
    category: 'System',
    shortcut: 'Ctrl+F',
    icon: 'search',
    execute: () => {
      ui().isSearchOpen = true;
    },
  });
}

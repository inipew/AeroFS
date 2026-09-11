import { onMounted, onUnmounted, type Ref } from 'vue';
import type { FileEntry } from '../../../types/vfs';

export interface UseFileKeyboardNavigationOptions {
  isActive: Ref<boolean>;
  viewMode: Ref<'list' | 'grid'>;
  displayedEntries: Ref<FileEntry[]>;
  selectedPaths: Ref<string[]>;
  gridCols: Ref<number>;
  onSelectIndex: (idx: number, isRange: boolean) => void;
  onActivate: (entry: FileEntry) => void;
  onNavigateUp: () => void;
  onRename: (entry: FileEntry) => void;
  onDelete: (paths: string[]) => void;
  onSelectAll: () => void;
  onClearSelection: () => void;
}

export function useFileKeyboardNavigation(options: UseFileKeyboardNavigationOptions) {
  function handleKeyDown(e: KeyboardEvent) {
    if (!options.isActive.value) return;

    const target = e.target as HTMLElement | null;
    if (
      target &&
      (target.tagName === 'INPUT' ||
        target.tagName === 'TEXTAREA' ||
        target.tagName === 'SELECT' ||
        target.isContentEditable)
    ) {
      return;
    }

    const entries = options.displayedEntries.value;
    const selected = options.selectedPaths.value;

    // Ctrl+A / Cmd+A -> Select All
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'a') {
      e.preventDefault();
      options.onSelectAll();
      return;
    }

    // Escape -> Clear Selection
    if (e.key === 'Escape') {
      e.preventDefault();
      options.onClearSelection();
      return;
    }

    // F2 -> Rename single selected
    if (e.key === 'F2') {
      e.preventDefault();
      if (selected.length === 1) {
        const item = entries.find((x) => x.path === selected[0]);
        if (item) options.onRename(item);
      }
      return;
    }

    // Delete -> Delete selection
    if (e.key === 'Delete') {
      e.preventDefault();
      if (selected.length > 0) {
        options.onDelete(selected);
      }
      return;
    }

    // Backspace -> Navigate Up
    if (e.key === 'Backspace') {
      e.preventDefault();
      options.onNavigateUp();
      return;
    }

    // Enter -> Activate
    if (e.key === 'Enter') {
      if (selected.length === 1) {
        e.preventDefault();
        const item = entries.find((x) => x.path === selected[0]);
        if (item) options.onActivate(item);
      }
      return;
    }

    // Space -> Quick Preview / Open
    if (e.key === ' ' && !e.shiftKey && !e.ctrlKey && !e.metaKey) {
      if (selected.length === 1) {
        e.preventDefault();
        const item = entries.find((x) => x.path === selected[0]);
        if (item) options.onActivate(item);
        return;
      }
    }

    // Arrow navigation
    if (['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(e.key)) {
      if (entries.length === 0) return;
      e.preventDefault();

      const isGrid = options.viewMode.value === 'grid';
      const step = isGrid ? Math.max(1, options.gridCols.value) : 1;

      if (e.key === 'ArrowDown') {
        if (selected.length === 0) {
          options.onSelectIndex(0, e.shiftKey);
        } else {
          const lastPath = selected[selected.length - 1];
          const idx = entries.findIndex((x) => x.path === lastPath);
          const nextIdx = Math.min(entries.length - 1, (idx >= 0 ? idx : 0) + step);
          options.onSelectIndex(nextIdx, e.shiftKey);
        }
      } else if (e.key === 'ArrowUp') {
        if (selected.length === 0) {
          options.onSelectIndex(entries.length - 1, e.shiftKey);
        } else {
          const firstPath = selected[0];
          const idx = entries.findIndex((x) => x.path === firstPath);
          const prevIdx = Math.max(0, (idx >= 0 ? idx : 0) - step);
          options.onSelectIndex(prevIdx, e.shiftKey);
        }
      } else if (e.key === 'ArrowRight') {
        if (isGrid) {
          if (selected.length === 0) {
            options.onSelectIndex(0, e.shiftKey);
          } else {
            const lastPath = selected[selected.length - 1];
            const idx = entries.findIndex((x) => x.path === lastPath);
            const nextIdx = Math.min(entries.length - 1, (idx >= 0 ? idx : 0) + 1);
            options.onSelectIndex(nextIdx, e.shiftKey);
          }
        } else {
          if (selected.length === 1) {
            const item = entries.find((x) => x.path === selected[0]);
            if (item) {
              options.onActivate(item);
            }
          } else if (selected.length === 0) {
            options.onSelectIndex(0, false);
          }
        }
      } else if (e.key === 'ArrowLeft') {
        if (isGrid) {
          if (selected.length === 0) {
            options.onSelectIndex(entries.length - 1, e.shiftKey);
          } else {
            const firstPath = selected[0];
            const idx = entries.findIndex((x) => x.path === firstPath);
            const prevIdx = Math.max(0, (idx >= 0 ? idx : 0) - 1);
            options.onSelectIndex(prevIdx, e.shiftKey);
          }
        } else {
          options.onNavigateUp();
        }
      }
    }
  }

  onMounted(() => {
    window.addEventListener('keydown', handleKeyDown);
  });

  onUnmounted(() => {
    window.removeEventListener('keydown', handleKeyDown);
  });
}

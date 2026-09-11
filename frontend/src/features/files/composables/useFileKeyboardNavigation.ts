import { onMounted, onUnmounted, type Ref } from 'vue';
import type { FileEntry } from '../../../types/vfs';

export interface UseFileKeyboardNavigationOptions {
  isActive: Ref<boolean>;
  viewMode: Ref<'list' | 'grid'>;
  displayedEntries: Ref<FileEntry[]>;
  selectedPaths: Ref<string[]>;
  activeCursorIndex?: Ref<number>;
  gridCols: Ref<number>;
  onSelectIndex: (idx: number, modifiers: { isRange: boolean; isMulti: boolean }) => void;
  onActivate: (entry: FileEntry) => void;
  onNavigateUp: () => void;
  onRename: (entry: FileEntry) => void;
  onDelete: (paths: string[], permanent?: boolean) => void;
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

    // Home -> Jump to first item
    if (e.key === 'Home') {
      if (entries.length === 0) return;
      e.preventDefault();
      options.onSelectIndex(0, { isRange: e.shiftKey, isMulti: e.ctrlKey || e.metaKey });
      return;
    }

    // End -> Jump to last item
    if (e.key === 'End') {
      if (entries.length === 0) return;
      e.preventDefault();
      options.onSelectIndex(entries.length - 1, { isRange: e.shiftKey, isMulti: e.ctrlKey || e.metaKey });
      return;
    }

    // Delete -> Delete selection (Shift+Del for permanent delete)
    if (e.key === 'Delete') {
      e.preventDefault();
      if (selected.length > 0) {
        options.onDelete(selected, e.shiftKey);
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

    // Arrow navigation: ArrowUp, ArrowDown, ArrowLeft, ArrowRight
    if (['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(e.key)) {
      if (entries.length === 0) return;
      e.preventDefault();

      const isGrid = options.viewMode.value === 'grid';
      const cols = isGrid ? Math.max(1, options.gridCols.value) : 1;
      const isMulti = e.ctrlKey || e.metaKey;
      const isRange = e.shiftKey;

      // Determine current index to navigate from
      let currentIdx = -1;
      if (
        options.activeCursorIndex &&
        options.activeCursorIndex.value >= 0 &&
        options.activeCursorIndex.value < entries.length
      ) {
        currentIdx = options.activeCursorIndex.value;
      } else if (selected.length > 0) {
        if (e.key === 'ArrowUp' || e.key === 'ArrowLeft') {
          const firstPath = selected[0];
          currentIdx = entries.findIndex((x) => x.path === firstPath);
        } else {
          const lastPath = selected[selected.length - 1];
          currentIdx = entries.findIndex((x) => x.path === lastPath);
        }
      }

      let nextIdx = 0;

      if (currentIdx === -1) {
        // No item currently selected or focused
        if (e.key === 'ArrowUp' || e.key === 'ArrowLeft') {
          nextIdx = entries.length - 1;
        } else {
          nextIdx = 0;
        }
      } else {
        if (e.key === 'ArrowDown') {
          nextIdx = Math.min(entries.length - 1, currentIdx + (isGrid ? cols : 1));
        } else if (e.key === 'ArrowUp') {
          nextIdx = Math.max(0, currentIdx - (isGrid ? cols : 1));
        } else if (e.key === 'ArrowRight') {
          nextIdx = Math.min(entries.length - 1, currentIdx + 1);
        } else if (e.key === 'ArrowLeft') {
          nextIdx = Math.max(0, currentIdx - 1);
        }
      }

      options.onSelectIndex(nextIdx, { isRange, isMulti });
    }
  }

  onMounted(() => {
    window.addEventListener('keydown', handleKeyDown);
  });

  onUnmounted(() => {
    window.removeEventListener('keydown', handleKeyDown);
  });
}

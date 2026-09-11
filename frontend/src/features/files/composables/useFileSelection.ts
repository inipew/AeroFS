import { ref, computed, type Ref } from 'vue';
import type { FileEntry } from '../../../types/vfs';

export interface UseFileSelectionOptions {
  entries: Ref<FileEntry[]>;
  selectedPaths: Ref<string[]>;
  onSelect: (paths: string[]) => void;
}

export function useFileSelection(options: UseFileSelectionOptions) {
  const lastClickedIndex = ref(-1);

  const selectedCount = computed(() => options.selectedPaths.value.length);
  const isAllSelected = computed(() => {
    if (options.entries.value.length === 0) return false;
    return options.entries.value.every((e) => options.selectedPaths.value.includes(e.path));
  });

  const selectedTotalSize = computed(() => {
    const selectedSet = new Set(options.selectedPaths.value);
    return options.entries.value
      .filter((e) => selectedSet.has(e.path))
      .reduce((sum, e) => sum + (e.size || 0), 0);
  });

  function clearSelection() {
    options.onSelect([]);
    lastClickedIndex.value = -1;
  }

  function selectAll() {
    if (isAllSelected.value) {
      clearSelection();
    } else {
      options.onSelect(options.entries.value.map((e) => e.path));
    }
  }

  function selectSingle(path: string) {
    const idx = options.entries.value.findIndex((e) => e.path === path);
    lastClickedIndex.value = idx;
    options.onSelect([path]);
  }

  function toggleItem(path: string) {
    const current = options.selectedPaths.value;
    const idx = options.entries.value.findIndex((e) => e.path === path);
    lastClickedIndex.value = idx;

    if (current.includes(path)) {
      options.onSelect(current.filter((p) => p !== path));
    } else {
      options.onSelect([...current, path]);
    }
  }

  function selectRange(targetIndex: number, additive: boolean = false) {
    if (targetIndex < 0 || targetIndex >= options.entries.value.length) return;
    const maxIdx = Math.max(0, options.entries.value.length - 1);
    const anchor = lastClickedIndex.value !== -1 ? Math.min(Math.max(0, lastClickedIndex.value), maxIdx) : 0;
    const start = Math.min(anchor, targetIndex);
    const end = Math.max(anchor, targetIndex);
    const rangePaths = options.entries.value.slice(start, end + 1).map((e) => e.path);

    if (additive) {
      options.onSelect(Array.from(new Set([...options.selectedPaths.value, ...rangePaths])));
    } else {
      options.onSelect(rangePaths);
    }
  }

  function handleEntrySelect(
    entry: FileEntry,
    modifiers?: { shiftKey?: boolean; ctrlKey?: boolean; metaKey?: boolean }
  ) {
    const currentIndex = options.entries.value.findIndex((e) => e.path === entry.path);
    const isShift = modifiers?.shiftKey ?? false;
    const isCtrl = (modifiers?.ctrlKey || modifiers?.metaKey) ?? false;

    if (isShift && currentIndex !== -1) {
      selectRange(currentIndex, isCtrl);
    } else if (isCtrl) {
      toggleItem(entry.path);
    } else {
      selectSingle(entry.path);
    }
  }

  function selectIndex(
    targetIndex: number,
    modifiers?: { isRange?: boolean; isMulti?: boolean }
  ) {
    if (targetIndex < 0 || targetIndex >= options.entries.value.length) return;
    const targetEntry = options.entries.value[targetIndex];
    if (!targetEntry) return;

    const isRange = modifiers?.isRange ?? false;
    const isMulti = modifiers?.isMulti ?? false;

    if (isRange) {
      selectRange(targetIndex, isMulti);
    } else if (isMulti) {
      const current = options.selectedPaths.value;
      if (!current.includes(targetEntry.path)) {
        options.onSelect([...current, targetEntry.path]);
      }
      lastClickedIndex.value = targetIndex;
    } else {
      selectSingle(targetEntry.path);
    }
  }

  return {
    lastClickedIndex,
    selectedCount,
    isAllSelected,
    selectedTotalSize,
    clearSelection,
    selectAll,
    selectSingle,
    toggleItem,
    selectRange,
    selectIndex,
    handleEntrySelect,
  };
}

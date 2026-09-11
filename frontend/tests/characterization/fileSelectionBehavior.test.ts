import { describe, expect, it } from 'bun:test';

describe('Characterization: File Selection Behavior', () => {
  const mockEntries = [
    { path: '/a.txt', name: 'a.txt' },
    { path: '/b.txt', name: 'b.txt' },
    { path: '/c.txt', name: 'c.txt' },
    { path: '/d.txt', name: 'd.txt' },
    { path: '/e.txt', name: 'e.txt' },
  ];

  interface SelectionState {
    selectedEntries: string[];
    lastClickedIndex: number;
  }

  function handleSelect(
    state: SelectionState,
    targetIndex: number,
    modifiers: { shiftKey?: boolean; ctrlOrMetaKey?: boolean }
  ) {
    const targetPath = mockEntries[targetIndex].path;

    if (modifiers.shiftKey && targetIndex !== -1) {
      const anchor = state.lastClickedIndex !== -1 ? state.lastClickedIndex : 0;
      const start = Math.min(anchor, targetIndex);
      const end = Math.max(anchor, targetIndex);
      const rangePaths = mockEntries.slice(start, end + 1).map((e) => e.path);

      if (modifiers.ctrlOrMetaKey) {
        state.selectedEntries = Array.from(new Set([...state.selectedEntries, ...rangePaths]));
      } else {
        state.selectedEntries = rangePaths;
      }
    } else if (modifiers.ctrlOrMetaKey) {
      if (state.selectedEntries.includes(targetPath)) {
        state.selectedEntries = state.selectedEntries.filter((p) => p !== targetPath);
      } else {
        state.selectedEntries.push(targetPath);
      }
      state.lastClickedIndex = targetIndex;
    } else {
      state.selectedEntries = [targetPath];
      state.lastClickedIndex = targetIndex;
    }
  }

  it('handles single selection replacing previous selection', () => {
    const state: SelectionState = { selectedEntries: ['/a.txt'], lastClickedIndex: 0 };
    handleSelect(state, 2, {});
    expect(state.selectedEntries).toEqual(['/c.txt']);
    expect(state.lastClickedIndex).toBe(2);
  });

  it('handles ctrl/meta toggle selection', () => {
    const state: SelectionState = { selectedEntries: ['/a.txt'], lastClickedIndex: 0 };
    handleSelect(state, 2, { ctrlOrMetaKey: true });
    expect(state.selectedEntries).toEqual(['/a.txt', '/c.txt']);

    // Toggle off
    handleSelect(state, 0, { ctrlOrMetaKey: true });
    expect(state.selectedEntries).toEqual(['/c.txt']);
  });

  it('handles shift range selection from anchor', () => {
    const state: SelectionState = { selectedEntries: ['/b.txt'], lastClickedIndex: 1 };
    handleSelect(state, 3, { shiftKey: true });
    expect(state.selectedEntries).toEqual(['/b.txt', '/c.txt', '/d.txt']);
  });

  it('handles shift + ctrl range selection preserving previous', () => {
    const state: SelectionState = { selectedEntries: ['/a.txt'], lastClickedIndex: 2 };
    handleSelect(state, 4, { shiftKey: true, ctrlOrMetaKey: true });
    expect(state.selectedEntries).toEqual(['/a.txt', '/c.txt', '/d.txt', '/e.txt']);
  });
});

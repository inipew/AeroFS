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

  it('navigates with arrow keys single selection without modifier', () => {
    const state: SelectionState = { selectedEntries: ['/a.txt'], lastClickedIndex: 0 };
    // Move to next item (ArrowDown / ArrowRight)
    handleSelect(state, 1, {});
    expect(state.selectedEntries).toEqual(['/b.txt']);
    expect(state.lastClickedIndex).toBe(1);

    // Move to another item
    handleSelect(state, 2, {});
    expect(state.selectedEntries).toEqual(['/c.txt']);
    expect(state.lastClickedIndex).toBe(2);
  });

  it('multi-selects additional files when Ctrl is held during keyboard navigation', () => {
    const state: SelectionState = { selectedEntries: ['/a.txt'], lastClickedIndex: 0 };
    // Navigate with Ctrl + Arrow
    handleSelect(state, 1, { ctrlOrMetaKey: true });
    expect(state.selectedEntries).toEqual(['/a.txt', '/b.txt']);
    expect(state.lastClickedIndex).toBe(1);

    handleSelect(state, 3, { ctrlOrMetaKey: true });
    expect(state.selectedEntries).toEqual(['/a.txt', '/b.txt', '/d.txt']);
    expect(state.lastClickedIndex).toBe(3);
  });

  it('calculates 2D grid index for Up, Down, Left, and Right arrow keys', () => {
    const total = 10;
    const cols = 4;

    function getNextGridIndex(current: number, key: string): number {
      switch (key) {
        case 'ArrowDown':
          return Math.min(total - 1, current + cols);
        case 'ArrowUp':
          return Math.max(0, current - cols);
        case 'ArrowRight':
          return Math.min(total - 1, current + 1);
        case 'ArrowLeft':
          return Math.max(0, current - 1);
        default:
          return current;
      }
    }

    expect(getNextGridIndex(0, 'ArrowDown')).toBe(4);
    expect(getNextGridIndex(4, 'ArrowDown')).toBe(8);
    expect(getNextGridIndex(8, 'ArrowDown')).toBe(9); // clamped to total - 1
    expect(getNextGridIndex(8, 'ArrowUp')).toBe(4);
    expect(getNextGridIndex(4, 'ArrowUp')).toBe(0);
    expect(getNextGridIndex(0, 'ArrowUp')).toBe(0); // clamped to 0
    expect(getNextGridIndex(0, 'ArrowRight')).toBe(1);
    expect(getNextGridIndex(1, 'ArrowLeft')).toBe(0);
  });

  it('handles Home and End keys jumping to first and last items', () => {
    const total = 12;
    function getJumpIndex(key: 'Home' | 'End', totalCount: number): number {
      return key === 'Home' ? 0 : Math.max(0, totalCount - 1);
    }

    expect(getJumpIndex('Home', total)).toBe(0);
    expect(getJumpIndex('End', total)).toBe(11);
    expect(getJumpIndex('Home', 0)).toBe(0);
    expect(getJumpIndex('End', 0)).toBe(0);
  });

  it('safely clamps anchor when entries change or index is out of bounds', () => {
    const entries = ['/a.txt', '/b.txt', '/c.txt'];
    const staleAnchor = 10;
    const maxIdx = Math.max(0, entries.length - 1);
    const clampedAnchor = Math.min(Math.max(0, staleAnchor), maxIdx);
    expect(clampedAnchor).toBe(2);

    const targetIdx = 1;
    const start = Math.min(clampedAnchor, targetIdx);
    const end = Math.max(clampedAnchor, targetIdx);
    const range = entries.slice(start, end + 1);
    expect(range).toEqual(['/b.txt', '/c.txt']);
  });
});

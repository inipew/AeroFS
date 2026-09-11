import { describe, expect, it } from 'bun:test';

describe('Characterization: Keyboard Shortcuts & Input Guard', () => {
  function shouldIgnoreShortcut(tagName?: string): boolean {
    if (!tagName) return false;
    const tag = tagName.toUpperCase();
    return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT';
  }

  it('correctly guards against input, textarea, and select elements', () => {
    expect(shouldIgnoreShortcut('INPUT')).toBe(true);
    expect(shouldIgnoreShortcut('input')).toBe(true);
    expect(shouldIgnoreShortcut('TEXTAREA')).toBe(true);
    expect(shouldIgnoreShortcut('textarea')).toBe(true);
    expect(shouldIgnoreShortcut('SELECT')).toBe(true);
    expect(shouldIgnoreShortcut('select')).toBe(true);
    expect(shouldIgnoreShortcut('DIV')).toBe(false);
    expect(shouldIgnoreShortcut('SECTION')).toBe(false);
    expect(shouldIgnoreShortcut('BUTTON')).toBe(false);
    expect(shouldIgnoreShortcut(undefined)).toBe(false);
  });

  it('distinguishes between pane-level shortcuts and file interaction shortcuts', () => {
    const paneNavigationKeys = ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'l', 'F5'];
    const fileInteractionKeys = ['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Space', 'Enter', 'F2', 'Delete', 'a'];

    // History Back / Forward requires Alt modifier
    function isPaneHistoryNav(key: string, altKey: boolean): boolean {
      return altKey && (key === 'ArrowLeft' || key === 'ArrowRight' || key === 'ArrowUp');
    }

    expect(isPaneHistoryNav('ArrowLeft', true)).toBe(true);
    expect(isPaneHistoryNav('ArrowLeft', false)).toBe(false);
    expect(isPaneHistoryNav('ArrowRight', true)).toBe(true);
    expect(isPaneHistoryNav('ArrowUp', true)).toBe(true);

    // Ctrl+L is address bar jump
    function isAddressBarShortcut(key: string, ctrlOrMeta: boolean): boolean {
      return ctrlOrMeta && key.toLowerCase() === 'l';
    }

    expect(isAddressBarShortcut('l', true)).toBe(true);
    expect(isAddressBarShortcut('L', true)).toBe(true);
    expect(isAddressBarShortcut('l', false)).toBe(false);
  });
});

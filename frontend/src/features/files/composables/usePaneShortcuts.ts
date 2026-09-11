import { onMounted, onUnmounted, type Ref } from 'vue';

export interface UsePaneShortcutsOptions {
  isActive: Ref<boolean>;
  isDualPane?: Ref<boolean>;
  onBack: () => void;
  onForward: () => void;
  onNavigateUp: () => void;
  onRefresh: () => void;
  onOpenAddressBar: () => void;
  onSwapPanels?: () => void;
  onSwitchActivePanel?: (target?: 'left' | 'right') => void;
  onCopy?: () => void;
  onCut?: () => void;
  onPaste?: () => void;
  onOpenInOtherPanel?: () => void;
}

export function usePaneShortcuts(options: UsePaneShortcutsOptions) {
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

    // Tab -> Switch Panel in Dual Pane Mode
    if (e.key === 'Tab' && options.isDualPane?.value && options.onSwitchActivePanel) {
      e.preventDefault();
      options.onSwitchActivePanel();
      return;
    }

    // Ctrl+Shift+Left / Cmd+Shift+Left -> Switch to Left Panel
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'ArrowLeft' && options.onSwitchActivePanel) {
      e.preventDefault();
      options.onSwitchActivePanel('left');
      return;
    }

    // Ctrl+Shift+Right / Cmd+Shift+Right -> Switch to Right Panel
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'ArrowRight' && options.onSwitchActivePanel) {
      e.preventDefault();
      options.onSwitchActivePanel('right');
      return;
    }

    // Alt+S -> Swap Panels
    if (e.altKey && e.key.toLowerCase() === 's' && options.onSwapPanels) {
      e.preventDefault();
      options.onSwapPanels();
      return;
    }

    // Ctrl+C / Cmd+C -> Copy Selection
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'c' && !e.shiftKey && options.onCopy) {
      e.preventDefault();
      options.onCopy();
      return;
    }

    // Ctrl+X / Cmd+X -> Cut Selection
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'x' && options.onCut) {
      e.preventDefault();
      options.onCut();
      return;
    }

    // Ctrl+V / Cmd+V -> Paste
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'v' && options.onPaste) {
      e.preventDefault();
      options.onPaste();
      return;
    }

    // Ctrl+Enter / Cmd+Enter -> Open in other panel
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter' && options.onOpenInOtherPanel) {
      e.preventDefault();
      options.onOpenInOtherPanel();
      return;
    }

    // Alt+Left -> Back
    if (e.altKey && e.key === 'ArrowLeft') {
      e.preventDefault();
      options.onBack();
      return;
    }

    // Alt+Right -> Forward
    if (e.altKey && e.key === 'ArrowRight') {
      e.preventDefault();
      options.onForward();
      return;
    }

    // Alt+Up -> Up
    if (e.altKey && e.key === 'ArrowUp') {
      e.preventDefault();
      options.onNavigateUp();
      return;
    }

    // Ctrl+L / Cmd+L -> Address Bar
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'l') {
      e.preventDefault();
      options.onOpenAddressBar();
      return;
    }

    // F5 -> Refresh
    if (e.key === 'F5') {
      e.preventDefault();
      options.onRefresh();
      return;
    }
  }

  onMounted(() => {
    window.addEventListener('keydown', handleKeyDown);
  });

  onUnmounted(() => {
    window.removeEventListener('keydown', handleKeyDown);
  });
}

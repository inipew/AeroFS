import { onMounted, onUnmounted } from 'vue';
import { useMediaViewerStore } from '../stores/mediaViewerStore';

export interface ViewerKeyboardActions {
  onClose?: () => void;
  onPrev?: () => void;
  onNext?: () => void;
  onTogglePlay?: () => void;
  onZoomIn?: () => void;
  onZoomOut?: () => void;
  onResetZoom?: () => void;
  onRotate?: () => void;
  onToggleFullscreen?: () => void;
}

export function useViewerKeyboard(actions: ViewerKeyboardActions = {}) {
  const store = useMediaViewerStore();

  function handleKeyDown(e: KeyboardEvent) {
    if (!store.isOpen) return;

    // Ignore when typing in input/textarea/editable
    const target = e.target as HTMLElement | null;
    if (
      target &&
      (target.tagName === 'INPUT' ||
        target.tagName === 'TEXTAREA' ||
        target.isContentEditable)
    ) {
      return;
    }

    switch (e.key) {
      case 'Escape':
        e.preventDefault();
        if (store.isInfoOpen) {
          store.toggleInfo(false);
        } else if (store.isMobileMenuOpen) {
          store.toggleMobileMenu(false);
        } else {
          actions.onClose ? actions.onClose() : store.close();
        }
        break;

      case 'ArrowLeft':
        e.preventDefault();
        actions.onPrev ? actions.onPrev() : store.navigate('prev');
        break;

      case 'ArrowRight':
        e.preventDefault();
        actions.onNext ? actions.onNext() : store.navigate('next');
        break;

      case ' ':
        e.preventDefault();
        actions.onTogglePlay?.();
        break;

      case '+':
      case '=':
        e.preventDefault();
        actions.onZoomIn?.();
        break;

      case '-':
      case '_':
        e.preventDefault();
        actions.onZoomOut?.();
        break;

      case '0':
        e.preventDefault();
        actions.onResetZoom?.();
        break;

      case 'r':
      case 'R':
        e.preventDefault();
        actions.onRotate?.();
        break;

      case 'i':
      case 'I':
        e.preventDefault();
        store.toggleInfo();
        break;

      case 'g':
      case 'G':
        e.preventDefault();
        store.toggleFilmstrip();
        break;

      case 'f':
      case 'F':
        e.preventDefault();
        actions.onToggleFullscreen?.();
        break;
    }
  }

  onMounted(() => {
    window.addEventListener('keydown', handleKeyDown);
  });

  onUnmounted(() => {
    window.removeEventListener('keydown', handleKeyDown);
  });

  return {
    handleKeyDown,
  };
}

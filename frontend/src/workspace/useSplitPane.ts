import { ref, type Ref, onUnmounted, getCurrentInstance } from 'vue';

export interface UseSplitPaneOptions {
  initialRatio?: number;
  min?: number; // default: 0.18
  max?: number; // default: 0.82
  onChange?: (ratio: number) => void;
}

/**
 * Decoupled desktop split-pane resize composable using unified Pointer Events.
 * Stores no Pinia state, uses rAF for 120fps CSS variable updates.
 */
export function useSplitPane(
  containerRef: Ref<HTMLElement | null>,
  options: UseSplitPaneOptions = {}
) {
  const minRatio = options.min ?? 0.18;
  const maxRatio = options.max ?? 0.82;
  const ratio = ref(options.initialRatio ?? 0.5);
  const isDragging = ref(false);

  let currentPendingRatio = ratio.value;
  let rafId: number | null = null;
  let activePointerId: number | null = null;
  let activeTarget: HTMLElement | null = null;

  function onPointerDown(e: PointerEvent) {
    e.preventDefault();
    isDragging.value = true;
    activePointerId = e.pointerId;
    activeTarget = e.currentTarget as HTMLElement;

    try {
      activeTarget?.setPointerCapture(e.pointerId);
    } catch {}

    if (typeof document !== 'undefined') {
      document.body.style.cursor = 'col-resize';
      document.body.style.userSelect = 'none';
    }

    if (typeof window !== 'undefined') {
      window.addEventListener('pointermove', onPointerMove, { passive: true });
      window.addEventListener('pointerup', onPointerUp);
      window.addEventListener('pointercancel', onPointerUp);
    }
  }

  function onPointerMove(e: PointerEvent) {
    if (!isDragging.value || !containerRef.value) return;
    const rect = containerRef.value.getBoundingClientRect();
    const rawRatio = (e.clientX - rect.left) / rect.width;
    const clamped = Math.max(minRatio, Math.min(maxRatio, rawRatio));
    currentPendingRatio = clamped;

    if (rafId) cancelAnimationFrame(rafId);
    rafId = requestAnimationFrame(() => {
      if (containerRef.value) {
        containerRef.value.style.setProperty('--split-ratio', `${clamped}`);
      }
    });
  }

  function onPointerUp(_e?: PointerEvent) {
    if (!isDragging.value) return;
    isDragging.value = false;

    if (rafId) cancelAnimationFrame(rafId);

    if (activeTarget && activePointerId !== null) {
      try {
        activeTarget.releasePointerCapture(activePointerId);
      } catch {}
    }
    activePointerId = null;
    activeTarget = null;

    if (typeof document !== 'undefined') {
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    }

    if (typeof window !== 'undefined') {
      window.removeEventListener('pointermove', onPointerMove);
      window.removeEventListener('pointerup', onPointerUp);
      window.removeEventListener('pointercancel', onPointerUp);
    }

    ratio.value = currentPendingRatio;
    options.onChange?.(currentPendingRatio);
  }

  if (getCurrentInstance()) {
    onUnmounted(() => {
      if (typeof window !== 'undefined') {
        window.removeEventListener('pointermove', onPointerMove);
        window.removeEventListener('pointerup', onPointerUp);
        window.removeEventListener('pointercancel', onPointerUp);
      }
      if (rafId) cancelAnimationFrame(rafId);
    });
  }

  return {
    ratio,
    isDragging,
    onPointerDown,
  };
}

import { ref, computed, type Ref } from 'vue';
import { getDynamicSettleDuration } from '../motion/tokens';

export interface UsePaneSwipeOptions {
  activePane: Ref<'left' | 'right'>;
  enabled: Ref<boolean>;
  containerRef: Ref<HTMLElement | null>;
  onChange: (pane: 'left' | 'right') => void;
}

/**
 * 1:1 verbatim extraction of AeroFS continuous dual-slide mobile swipe gesture tracking.
 * Features: direction locking, velocity tracking, rubber-band resistance, dynamic settle spring.
 */
export function usePaneSwipe(options: UsePaneSwipeOptions) {
  const isDragging = ref(false);
  const dragDeltaX = ref(0);
  const settleDuration = ref(320);

  let touchStartX = 0;
  let touchStartY = 0;
  let touchStartTime = 0;
  let lastTouchX = 0;
  let lastTouchTime = 0;
  let touchVelocityX = 0; // px / ms
  let isHorizontalGesture: boolean | null = null;
  let activeRafId: number | null = null;

  const trackStyle = computed(() => {
    const baseOffset = options.activePane.value === 'left' ? 0 : -50;

    if (isDragging.value) {
      // When actively dragging, translate continuously with live delta
      return {
        transform: `translate3d(calc(${baseOffset}% + ${dragDeltaX.value}px), 0, 0)`,
        transition: 'none',
      };
    }

    // When settling or toggled via button, apply Apple spring curve with velocity-dynamic duration
    return {
      transform: `translate3d(${baseOffset}%, 0, 0)`,
      transition: `transform ${settleDuration.value}ms cubic-bezier(0.32, 0.72, 0, 1)`,
    };
  });

  function onTouchStart(e: TouchEvent) {
    if (!options.enabled.value || e.touches.length === 0) return;
    const touch = e.touches[0];
    touchStartX = touch.clientX;
    touchStartY = touch.clientY;
    touchStartTime = performance.now();
    lastTouchX = touch.clientX;
    lastTouchTime = touchStartTime;
    touchVelocityX = 0;
    isHorizontalGesture = null;
    dragDeltaX.value = 0;
    settleDuration.value = 320;
  }

  function onTouchMove(e: TouchEvent) {
    if (!options.enabled.value || e.touches.length === 0) return;
    const touch = e.touches[0];
    const dx = touch.clientX - touchStartX;
    const dy = touch.clientY - touchStartY;

    // Determine gesture direction on initial movement
    if (isHorizontalGesture === null) {
      if (Math.abs(dx) > 6 || Math.abs(dy) > 6) {
        isHorizontalGesture = Math.abs(dx) > Math.abs(dy) * 1.15;
      }
    }

    if (isHorizontalGesture) {
      e.preventDefault(); // Lock vertical scroll during horizontal panel swipe
      isDragging.value = true;

      const now = performance.now();
      const dt = now - lastTouchTime;
      if (dt > 10) {
        touchVelocityX = (touch.clientX - lastTouchX) / dt;
        lastTouchX = touch.clientX;
        lastTouchTime = now;
      }

      // Apply Apple-style Rubber-Banding Resistance when overscrolling boundaries
      let effectiveDx = dx;
      if (options.activePane.value === 'left' && dx > 0) {
        effectiveDx = dx * 0.3;
      } else if (options.activePane.value === 'right' && dx < 0) {
        effectiveDx = dx * 0.3;
      }

      if (activeRafId) cancelAnimationFrame(activeRafId);
      activeRafId = requestAnimationFrame(() => {
        dragDeltaX.value = effectiveDx;
      });
    }
  }

  function onTouchEnd() {
    if (!options.enabled.value) return;
    if (activeRafId) cancelAnimationFrame(activeRafId);

    if (isDragging.value) {
      isDragging.value = false;
      const finalDeltaX = dragDeltaX.value;
      dragDeltaX.value = 0;

      // Calculate momentum-driven spring settle duration based on touch release velocity
      settleDuration.value = getDynamicSettleDuration(touchVelocityX, 320, 180);

      const containerWidth = options.containerRef.value?.clientWidth || (typeof window !== 'undefined' ? window.innerWidth : 360);
      const distanceThreshold = containerWidth * 0.26;
      const velocityThreshold = 0.4; // px / ms

      // Fast velocity flick or passed distance threshold
      if (options.activePane.value === 'left') {
        if (finalDeltaX < -distanceThreshold || touchVelocityX < -velocityThreshold) {
          options.onChange('right');
        }
      } else if (options.activePane.value === 'right') {
        if (finalDeltaX > distanceThreshold || touchVelocityX > velocityThreshold) {
          options.onChange('left');
        }
      }
    }

    isHorizontalGesture = null;
  }

  function onTouchCancel() {
    if (isDragging.value) {
      isDragging.value = false;
      dragDeltaX.value = 0;
    }
    isHorizontalGesture = null;
  }

  return {
    isDragging,
    trackStyle,
    onTouchStart,
    onTouchMove,
    onTouchEnd,
    onTouchCancel,
  };
}

import { describe, expect, it } from 'bun:test';
import { ref } from 'vue';
import { usePaneSwipe } from '../../src/workspace/usePaneSwipe';

describe('usePaneSwipe composable', () => {
  it('computes initial trackStyle correctly', () => {
    const activePane = ref<'left' | 'right'>('left');
    const enabled = ref(true);
    const containerRef = ref<HTMLElement | null>(null);

    const swipe = usePaneSwipe({
      activePane,
      enabled,
      containerRef,
      onChange: (p) => { activePane.value = p; },
    });

    expect(swipe.trackStyle.value.transform).toContain('0%');
    expect(swipe.isDragging.value).toBe(false);

    activePane.value = 'right';
    expect(swipe.trackStyle.value.transform).toContain('-50%');
  });

  it('cancels drag cleanly on onTouchCancel', () => {
    const activePane = ref<'left' | 'right'>('left');
    const enabled = ref(true);
    const containerRef = ref<HTMLElement | null>(null);

    const swipe = usePaneSwipe({
      activePane,
      enabled,
      containerRef,
      onChange: () => {},
    });

    swipe.onTouchCancel();
    expect(swipe.isDragging.value).toBe(false);
  });
});

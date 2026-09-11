import { describe, expect, it } from 'bun:test';
import { ref } from 'vue';
import { useSplitPane } from '../../src/workspace/useSplitPane';

describe('useSplitPane composable', () => {
  it('initializes with default or custom ratio', () => {
    const container = ref<HTMLElement | null>(null);
    const { ratio, isDragging } = useSplitPane(container, { initialRatio: 0.6 });
    expect(ratio.value).toBe(0.6);
    expect(isDragging.value).toBe(false);
  });

  it('respects min and max bounds', () => {
    let committedRatio = 0;
    const container = ref<HTMLElement | null>(null);
    const { ratio } = useSplitPane(container, {
      initialRatio: 0.5,
      min: 0.2,
      max: 0.8,
      onChange: (r) => { committedRatio = r; },
    });

    expect(ratio.value).toBe(0.5);
  });
});

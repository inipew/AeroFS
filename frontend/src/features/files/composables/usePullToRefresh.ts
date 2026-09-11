import { ref, type Ref } from 'vue';

export interface UsePullToRefreshOptions {
  containerRef: Ref<HTMLElement | null>;
  enabled: Ref<boolean>;
  onRefresh: () => Promise<void> | void;
}

export function usePullToRefresh(options: UsePullToRefreshOptions) {
  const isRefreshing = ref(false);
  const isPulling = ref(false);
  const pullDistance = ref(0);
  let startY = 0;

  function onTouchStart(e: TouchEvent) {
    if (!options.enabled.value || !options.containerRef.value) return;
    if (options.containerRef.value.scrollTop <= 0) {
      startY = e.touches[0].clientY;
      isPulling.value = true;
    }
  }

  function onTouchMove(e: TouchEvent) {
    if (!isPulling.value || isRefreshing.value || !options.containerRef.value) return;
    if (options.containerRef.value.scrollTop <= 0) {
      const currentY = e.touches[0].clientY;
      const diff = currentY - startY;
      if (diff > 0) {
        pullDistance.value = Math.min(70, diff * 0.45);
      }
    } else {
      isPulling.value = false;
      pullDistance.value = 0;
    }
  }

  async function onTouchEnd() {
    if (pullDistance.value >= 40 && !isRefreshing.value) {
      isRefreshing.value = true;
      pullDistance.value = 45;
      try {
        await options.onRefresh();
      } finally {
        setTimeout(() => {
          isRefreshing.value = false;
          pullDistance.value = 0;
          isPulling.value = false;
        }, 250);
      }
    } else {
      pullDistance.value = 0;
      isPulling.value = false;
    }
  }

  return {
    isRefreshing,
    isPulling,
    pullDistance,
    onTouchStart,
    onTouchMove,
    onTouchEnd,
  };
}

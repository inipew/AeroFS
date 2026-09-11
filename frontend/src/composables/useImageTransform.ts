import { ref, computed } from 'vue';

export interface ImageTransformOptions {
  minZoom?: number;
  maxZoom?: number;
  zoomStep?: number;
}

export function useImageTransform(options: ImageTransformOptions = {}) {
  const minZoom = options.minZoom ?? 0.25;
  const maxZoom = options.maxZoom ?? 5.0;
  const zoomStep = options.zoomStep ?? 0.25;

  const zoomLevel = ref<number>(1);
  const rotation = ref<number>(0);
  const panX = ref<number>(0);
  const panY = ref<number>(0);
  const isPanning = ref<boolean>(false);

  let startClientX = 0;
  let startClientY = 0;

  // Touch pinch-to-zoom tracking
  let initialPinchDistance = 0;
  let initialPinchZoom = 1;

  function clampZoom(val: number): number {
    return Math.max(minZoom, Math.min(maxZoom, Math.round(val * 100) / 100));
  }

  function zoomIn(step = zoomStep) {
    zoomLevel.value = clampZoom(zoomLevel.value + step);
  }

  function zoomOut(step = zoomStep) {
    zoomLevel.value = clampZoom(zoomLevel.value - step);
    if (zoomLevel.value <= 1) {
      panX.value = 0;
      panY.value = 0;
    }
  }

  function setZoom(val: number) {
    zoomLevel.value = clampZoom(val);
    if (zoomLevel.value <= 1) {
      panX.value = 0;
      panY.value = 0;
    }
  }

  function resetZoom() {
    zoomLevel.value = 1;
    panX.value = 0;
    panY.value = 0;
  }

  function rotateClockwise() {
    rotation.value = (rotation.value + 90) % 360;
  }

  function rotateCounterClockwise() {
    rotation.value = (rotation.value - 90 + 360) % 360;
  }

  function resetTransform() {
    zoomLevel.value = 1;
    rotation.value = 0;
    panX.value = 0;
    panY.value = 0;
    isPanning.value = false;
    initialPinchDistance = 0;
  }

  function handleWheel(e: WheelEvent) {
    e.preventDefault();
    const delta = -e.deltaY;
    if (delta > 0) {
      zoomIn(0.15);
    } else {
      zoomOut(0.15);
    }
  }

  function handleDoubleClick() {
    if (zoomLevel.value > 1.05) {
      resetZoom();
    } else {
      setZoom(2.0);
    }
  }

  function getTouchDistance(e: TouchEvent): number {
    if (e.touches.length < 2) return 0;
    const dx = e.touches[0].clientX - e.touches[1].clientX;
    const dy = e.touches[0].clientY - e.touches[1].clientY;
    return Math.hypot(dx, dy);
  }

  function startPan(e: MouseEvent | TouchEvent) {
    if ('touches' in e && e.touches.length === 2) {
      initialPinchDistance = getTouchDistance(e);
      initialPinchZoom = zoomLevel.value;
      isPanning.value = false;
      return;
    }

    if (zoomLevel.value <= 1) return;
    isPanning.value = true;
    const clientX = 'touches' in e ? e.touches[0].clientX : e.clientX;
    const clientY = 'touches' in e ? e.touches[0].clientY : e.clientY;
    startClientX = clientX - panX.value;
    startClientY = clientY - panY.value;
  }

  function doPan(e: MouseEvent | TouchEvent) {
    if ('touches' in e && e.touches.length === 2 && initialPinchDistance > 0) {
      const currentDist = getTouchDistance(e);
      if (currentDist > 0) {
        const factor = currentDist / initialPinchDistance;
        setZoom(initialPinchZoom * factor);
      }
      return;
    }

    if (!isPanning.value) return;
    const clientX = 'touches' in e ? e.touches[0].clientX : e.clientX;
    const clientY = 'touches' in e ? e.touches[0].clientY : e.clientY;
    panX.value = clientX - startClientX;
    panY.value = clientY - startClientY;
  }

  function stopPan() {
    isPanning.value = false;
    initialPinchDistance = 0;
  }

  // Critical Invariant: translate3d MUST precede scale and rotate
  // to preserve screen-space pan alignment regardless of image rotation.
  const transformStyle = computed(() => {
    return {
      transform: `translate3d(${panX.value}px, ${panY.value}px, 0px) scale(${zoomLevel.value}) rotate(${rotation.value}deg)`,
      transition: isPanning.value ? 'none' : 'transform 0.15s ease-out',
    };
  });

  return {
    zoomLevel,
    rotation,
    panX,
    panY,
    isPanning,
    zoomIn,
    zoomOut,
    setZoom,
    resetZoom,
    rotateClockwise,
    rotateCounterClockwise,
    resetTransform,
    handleWheel,
    handleDoubleClick,
    startPan,
    doPan,
    stopPan,
    transformStyle,
  };
}

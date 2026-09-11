<template>
  <div
    @pointerdown="$emit('pointerdown', $event)"
    :class="[
      'w-1.5 hover:w-2 transition-[width,background-color] duration-150 cursor-col-resize flex items-center justify-center shrink-0 z-20 group relative touch-none select-none',
      isDragging ? 'bg-blue-600 w-2' : 'hover:bg-blue-500/40'
    ]"
    title="Drag to resize split panes"
    role="separator"
    aria-orientation="vertical"
    tabindex="0"
  >
    <div
      :class="[
        'w-0.5 h-8 rounded-full transition-colors',
        isDragging ? 'bg-white' : 'bg-gray-300 dark:bg-slate-700 group-hover:bg-blue-500'
      ]"
    ></div>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  isDragging?: boolean;
}>();

defineEmits<{
  (e: 'pointerdown', event: PointerEvent): void;
}>();
</script>

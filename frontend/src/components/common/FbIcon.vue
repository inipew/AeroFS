<template>
  <span
    class="fb-icon inline-flex items-center justify-center shrink-0 leading-none align-middle"
    :style="{ width: size, height: size, fontSize: size }"
    :aria-label="label"
    :aria-hidden="!label"
    v-html="svg"
  />
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { icons, type IconName } from '../../utils/icons';

const props = withDefaults(
  defineProps<{
    name: IconName;
    size?: string;
    label?: string;
  }>(),
  { size: '1.25em' }
);

const svg = computed(() => {
  const paths = icons[props.name] ?? icons['file'];
  return `<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="100%" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${paths}</svg>`;
});
</script>

<style scoped>
.fb-icon :deep(svg) {
  display: block;
  width: 100%;
  height: 100%;
}
</style>

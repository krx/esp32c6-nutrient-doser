<script setup lang="ts">
import * as api from '@/utils/api';
import type { DoserInfo } from '#shared/types/doser';
import type { Chart } from '~/types/feedchart';

const dosers = ref<DoserInfo[]>(await api.get_dosers());
const selected = ref<DoserInfo | undefined>();

const all_charts: { [key: string]: Chart } = [
  (await import('@/assets/floranova.json')) as Chart,
  (await import('@/assets/maxiseries_indoor.json')) as Chart,
  (await import('@/assets/maxiseries_outdoor.json')) as Chart,
  (await import('@/assets/floraseries.json')) as Chart,
].reduce((obj, c) => ({ ...obj, [c.name]: c }), {});

const nutrients = new Set<string>();
Object.values(all_charts).forEach((_chart) => {
  _chart.nutrients.forEach((n) => {
    nutrients.add(n.name);
  });
});
</script>

<template>
  <UContainer class="flex flex-col items-center justify-center gap-4 overflow-y-auto my-8 max-w-2xl">
    <h1 class="font-bold text-2xl text-primary">Nutrient Doser</h1>

    <div v-show="$pwa?.needRefresh">
      <span> New content available, click on reload button to update. </span>

      <button @click="$pwa?.updateServiceWorker()">Reload</button>
    </div>

    <AddDoser v-if="dosers.length === 0" v-model="dosers" v-model:all-charts="all_charts" />
    <SelectDoser
      v-else
      v-model:dosers="dosers"
      v-model:selected="selected"
      v-model:all-charts="all_charts"
    />

    <DoserView
      v-if="selected !== undefined"
      :key="selected.url"
      v-model="selected"
      v-model:all-charts="all_charts"
      v-model:nutrients="nutrients"
    />
  </UContainer>
</template>

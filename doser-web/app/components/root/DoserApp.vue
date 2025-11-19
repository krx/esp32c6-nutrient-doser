<script setup lang="ts">
import * as api from '@/utils/api';
import type { DoserInfo } from '#shared/types/doser';
import type { Chart } from '~/types/feedchart';

const dosers = ref<DoserInfo[]>(await api.get_dosers());
const selected = defineModel<DoserInfo | undefined>();
const all_charts = defineModel<{ [key: string]: Chart }>('all-charts', { required: true });
</script>

<template>
  <UContainer class="flex flex-col items-center justify-center gap-4 overflow-y-auto my-8 max-w-2xl">
    <h1 class="font-bold text-2xl text-primary">Nutrient Doser</h1>

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
    />
  </UContainer>
</template>

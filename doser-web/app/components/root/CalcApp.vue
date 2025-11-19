<script setup lang="ts">
import _ from 'lodash';
import type { Chart, VolUnit } from '~/types/feedchart';

const all_charts = defineModel<{ [key: string]: Chart }>('all-charts', { required: true });

const chart = ref<string>(Object.keys(all_charts.value)[0]!);
const schedule = ref(default_schedule());
const stage = ref(default_stage());

const target_amount = ref(1.0);
const target_unit = ref<VolUnit>('gal');
const units = ref<VolUnit[]>(['mL', 'L', 'fl oz', 'gal']);

function default_schedule(): string {
  return Object.keys(all_charts.value[chart.value]!.charts)[0]!;
}

function default_stage(): string {
  return Object.keys(all_charts.value[chart.value]!.charts[schedule.value]!)[0]!;
}
</script>

<template>
  <UContainer class="flex flex-col items-center justify-center gap-4 overflow-y-auto my-8 max-w-2xl">
    <h1 class="font-bold text-2xl text-primary">Nutrient Calculator</h1>
    <div class="flex flex-col w-full gap-4">
      <UCard variant="subtle">
        <UContainer class="flex flex-col gap-2 items-center justify-center">
          <UFieldGroup class="w-full">
            <UBadge label="Feed Chart" color="neutral" variant="subtle" size="xl" />
            <USelect
              v-model="chart"
              :items="Object.values(all_charts).map((c) => c.name)"
              class="w-full"
              placeholder="Select a feedchart"
              @change="stage = default_stage()"
            />
          </UFieldGroup>
          <TabSelect
            v-model="schedule"
            :items="
            Object.keys(all_charts[chart]!.charts).map((k) => {
              return { label: _.capitalize(k), value: k };
            })
          "
          />
          <TabSelect
            v-model="stage"
            :items="
            Object.keys(all_charts[chart]!.charts[schedule]!).map((k) => {
              return { label: _.capitalize(k), value: k };
            })
          "
            :ui="{
              label: 'text-clip text-pretty',
            }"
          />

          <UFieldGroup class="w-full">
            <UBadge label="Target amount" color="neutral" variant="subtle" size="xl" />
            <UInputNumber v-model="target_amount" :min="0" :step="0.1" class="w-full" size="xl" />
            <USelect
              v-model="target_unit"
              :items="units"
              color="neutral"
              variant="subtle"
              arrow
              size="xl"
            />
          </UFieldGroup>
        </UContainer>
      </UCard>

      <NutrientChart
        v-model:chart="all_charts[chart]!"
        v-model:stage="all_charts[chart]!.charts[schedule]![stage]!"
        v-model:amount="target_amount"
        v-model:unit="target_unit"
      />
    </div>
  </UContainer>
</template>

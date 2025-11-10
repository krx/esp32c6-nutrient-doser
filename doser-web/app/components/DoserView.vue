<script setup lang="ts">
import * as dapi from '@/utils/doser_api';
import type { DoserInfo } from '#shared/types/doser';
import type { DoseSolutionReq } from '@/types/doser_api';
import _ from 'lodash';
import type { Chart, VolUnit } from '~/types/feedchart';

const doser = defineModel<DoserInfo>({ required: true });
const all_charts = defineModel<{ [key: string]: Chart }>('all-charts', { required: true });

const chart = ref(all_charts.value[doser.value.chart]!);
const schedule = ref(default_schedule());
const stage = ref(default_stage());

function default_schedule(): string {
  return Object.keys(chart.value!.charts)[0]!;
}

function default_stage(): string {
  return Object.keys(chart.value!.charts[schedule.value]!)[0]!;
}

const target_amount = ref(1.0);
const target_unit = ref<VolUnit>('gal');
const units = ref<VolUnit[]>(['mL', 'L', 'gal', 'fl oz']);

async function dispense_solution() {
  const table = chart.value!.charts[schedule.value]![stage.value]!;
  const req: DoseSolutionReq = {
    nutrients: Object.entries(table).map(([nutrient, amount]) => {
      return {
        name: nutrient,
        ml_per_gal: amount,
        motor_idx: doser.value.motors.find((m) => m.name == nutrient)!.idx,
      };
    }),
    target_amount: target_amount.value,
    target_unit: target_unit.value,
  };
  await dapi.dose_solution(doser.value.url, req);
}
</script>

<template>
  <div class="flex flex-col w-full gap-4">
    <UCard variant="subtle">
      <template #header>
        <UContainer class="flex items-center justify-center">
          <h1 class="font-bold text-xl">Motors</h1>
        </UContainer>
      </template>

      <div class="flex flex-wrap items-center justify-center gap-2 max-w-(--ui-container) mx-auto px-4">
        <MotorCard
          v-for="motor in doser.motors"
          :key="motor.idx"
          v-model="doser.motors[motor.idx]!"
          v-model:doser="doser"
        />
      </div>

      <template #footer>
        <UContainer class="flex items-center justify-center">
          <UButton
            loading-auto
            label="Unprime all motors"
            icon="i-lucide-droplet-off"
            size="xl"
            @click="dapi.unprime_all(doser.url)"
          />
        </UContainer>
      </template>
    </UCard>

    <UCard variant="subtle">
      <template #header>
        <UContainer class="flex items-center justify-center">
          <h1 class="font-bold text-xl">Dispense Solution</h1>
        </UContainer>
      </template>
      <UContainer class="flex flex-col gap-2 items-center justify-center">
        <UTabs
          v-model="schedule"
          class="w-full"
          size="xl"
          :items="
            Object.keys(chart!.charts).map((k) => {
              return { label: _.capitalize(k), value: k };
            })
          "
          :content="false"
        />
        <UTabs
          v-model="stage"
          class="w-full"
          size="xl"
          :items="
            Object.keys(chart!.charts[schedule]!).map((k) => {
              return { label: _.capitalize(k), value: k };
            })
          "
          :ui="{
            label: 'text-xs sm:text-base text-wrap',
          }"
          :content="false"
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
      <template #footer>
        <UContainer class="flex items-center justify-center">
          <UButton
            label="Dispense"
            icon="i-lucide-wand-sparkles"
            size="xl"
            loading-auto
            @click="dispense_solution"
          />
        </UContainer>
      </template>
    </UCard>

    <NutrientChart
      v-model:chart="chart"
      v-model:stage="chart.charts[schedule]![stage]!"
      v-model:amount="target_amount"
      v-model:unit="target_unit"
    />
  </div>
</template>

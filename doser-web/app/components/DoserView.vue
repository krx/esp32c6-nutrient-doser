<script setup lang="ts">
import * as api from '@/utils/api';
import * as dapi from '@/utils/doser_api';
import raw_chart from '@/assets/floragro_chart.json';
import type { DoserInfo } from '#shared/types/doser';
import type { FeedChart, Nutrient } from '#shared/types/feedchart';
import type { DoseSolutionReq, VolUnit } from '@/types/doser_api';
import _ from 'lodash';

const doser = defineModel<DoserInfo>({ required: true });

const chart = raw_chart as FeedChart;
const schedule = ref<string>(Object.keys(chart)[1]!);
const stage = ref<string>(Object.keys(chart[schedule.value]!)[0]!);
const nutrients = new Set<Nutrient>();
Object.values(chart).forEach((_sched) => {
  Object.values(_sched).forEach((_stage) => {
    Object.keys(_stage).forEach((k) => nutrients.add(k));
  });
});

const { num_motors } = await dapi.get_info(doser.value.url);
if (num_motors !== doser.value.motors.length) {
  doser.value.motors = [];
  _.range(num_motors).forEach((i) => {
    doser.value.motors.push({
      idx: i,
      prime_ml: 1.0,
    });
  });
}

function all_motors_configured() {
  return doser.value.motors.every((m) => m.name);
}

async function prime_all() {
  await api.add_doser(doser.value);
  await dapi.dispense(doser.value.url, {
    reqs: doser.value.motors.map((m) => {
      return {
        motor_idx: m.idx,
        ml: m.prime_ml!,
      };
    }),
  });
}

const target_amount = ref(1.0);
const target_unit = ref<VolUnit>('gal');
const units = ref<VolUnit[]>(['mL', 'L', 'gal']);

async function dispense_solution() {
  const table = chart[schedule.value]![stage.value]!;
  const req: DoseSolutionReq = {
    nutrients: Object.entries(table).map(([nutrient, amount]) => {
      return {
        name: nutrient,
        ml_per_gal: amount,
        motor_idx: doser.value.motors.find((m) => m.name == nutrient)!.idx,
      };
    }),
    target_amount: target_amount.value,
    target_unit: _.capitalize(target_unit.value),
  };
  await dapi.dose_solution(doser.value.url, req);
}
</script>

<template>
  <UContainer v-if="!all_motors_configured()">
    <h2>The following motors need to have a nutrient assigned:</h2>
    <UButtonGroup
      v-for="motor in doser.motors.filter((m) => !m.name)"
      :key="motor.idx"
      class="w-full"
    >
      <UBadge :label="`Motor #${motor.idx}`" />
      <USelect
        v-model="motor.name"
        :items="Array.from(nutrients)"
        class="w-full"
        @change="api.add_doser(doser)"
      />
    </UButtonGroup>
  </UContainer>

  <div v-else class="flex flex-col w-full gap-4">
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
            label="Prime all motors"
            icon="i-lucide-droplets"
            size="xl"
            @click="prime_all"
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
            Object.keys(chart).map((k) => {
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
            Object.keys(chart[schedule]!).map((k) => {
              return { label: _.capitalize(k), value: k };
            })
          "
          :ui="{
            label: 'text-xs sm:text-base text-wrap',
          }"
          :content="false"
        />

        <UButtonGroup class="w-full">
          <UBadge label="Target amount" color="neutral" variant="subtle" size="xl" />
          <UInputNumber v-model="target_amount" :min="0" class="w-full" size="xl" />
          <USelect
            v-model="target_unit"
            :items="units"
            color="neutral"
            variant="subtle"
            arrow
            size="xl"
          />
        </UButtonGroup>
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
  </div>
</template>

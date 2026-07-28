<script setup lang="ts">
import * as dapi from '@/utils/doser_api';
import type { StepperItem } from '@nuxt/ui';
import type { DoserInfo, MotorConfig } from '~~/shared/types/doser';
import type { NutrientTable, NutrientUnit } from '~/types/nutrients';
import nutrients from '@/assets/nutrients.json';

const motor = defineModel<MotorConfig>({ required: true });
const doser = defineModel<DoserInfo>('doser', { required: true });
const modal_open = defineModel<boolean>('modal', { required: true });
const wakelock = reactive(useWakeLock());

const dispensed = ref(10.0);
const actual = ref(10.0);
const actual_unit = ref<NutrientUnit>('g');
const nutrient_table = nutrients as NutrientTable;
const nutrient_density = computed(() => nutrient_table.find((nutrient) => nutrient.name === motor.value.name)?.density);
const actual_units = computed<NutrientUnit[]>(() => nutrient_density.value ? ['mL', 'g'] : ['mL']);
const actual_ml = computed(() => actual_unit.value === 'g' ? actual.value / nutrient_density.value! : actual.value);

const stepper = useTemplateRef('stepper');
const active_step = ref(0);
const steps = ref<StepperItem[]>([
  {
    slot: 'dispense',
    title: 'Dispense',
    description: 'Dispense a test amount',
  },
  {
    slot: 'calibrate',
    title: 'Calibrate',
    description: 'Report actual amount',
  },
  {
    slot: 'confirm',
    title: 'Confirm',
    description: 'Confirm calibration',
  },
]);

async function dispense_step() {
  await wakelock.request('screen');
  await dapi.dispense(doser.value.url, {
    reqs: [
      {
        motor_idx: motor.value.idx,
        ml: dispensed.value,
      },
    ],
  });
  actual.value = dispensed.value;
  stepper.value?.next();
  await wakelock.release();
}

async function calibrate_step() {
  await dapi.calibrate(doser.value.url, {
    motor_idx: motor.value.idx,
    expected: dispensed.value,
    actual: actual_ml.value,
  });
  stepper.value?.next();
}

async function confirm_step() {
  await wakelock.request('screen');
  await dapi.dispense(doser.value.url, {
    reqs: [
      {
        motor_idx: motor.value.idx,
        ml: dispensed.value,
      },
    ],
  });
  await wakelock.release();
}
</script>

<template>
  <UContainer class="">
    <UStepper ref="stepper" v-model="active_step" :items="steps" disabled>
      <template #dispense>
        <div class="flex flex-col justify-center items-center gap-4">
          Select a test amount of liquid to dispense
          <h1 class="text-4xl">{{ dispensed }} mL</h1>
          <USlider v-model="dispensed" :min="0.1" :max="20.0" :step="0.1" />
          <USeparator class="h-4" />
          <div class="flex flex-row w-full">
            <UButton label="Cancel" icon="i-lucide-x" @click="modal_open = false" />
            <UButton
              class="ml-auto"
              label="Dispense"
              trailing-icon="i-lucide-arrow-right"
              loading-auto
              @click="dispense_step"
            />
          </div>
        </div>
      </template>
      <template #calibrate>
        <div class="flex flex-col justify-center items-center gap-4">
          Enter how much liquid was dispensed
          <UFieldGroup size="xl">
            <UInputNumber
              v-model="actual"
              :min="0.0"
              :step="0.1"
              :step-snapping="false"
              :ui="{ base: ['text-4xl'] }"
            />
            <USelect v-model="actual_unit" :items="actual_units" variant="subtle" color="neutral" />
          </UFieldGroup>
          <p v-if="actual_unit === 'g'" class="text-sm text-muted">
            {{ actual }} g = {{ actual_ml.toFixed(2) }} mL ({{ nutrient_density }} g/mL)
          </p>
          <USeparator class="h-4" />
          <div class="flex flex-row w-full">
            <UButton label="Back" icon="i-lucide-arrow-left" @click="stepper?.prev()" />
            <UButton
              class="ml-auto"
              label="Calibrate"
              trailing-icon="i-lucide-arrow-right"
              loading-auto
              @click="calibrate_step"
            />
          </div>
        </div>
      </template>
      <template #confirm>
        <div class="flex flex-col justify-center items-center gap-4">
          Confirm that the right amount is dispensed after calibrating
          <UButton
            :label="`Dispense ${dispensed} mL`"
            icon="i-lucide-droplet"
            loading-auto
            @click="confirm_step"
          />
          <USeparator class="h-4" />
          <div class="flex flex-row w-full">
            <UButton label="Restart" icon="i-lucide-rotate-ccw" @click="active_step = 0" />
            <UButton
              class="ml-auto"
              label="Done"
              trailing-icon="i-lucide-check"
              loading-auto
              @click="modal_open = false"
            />
          </div>
        </div>
      </template>
    </UStepper>
  </UContainer>
</template>

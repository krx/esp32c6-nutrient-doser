<script setup lang="ts">
import * as api from '@/utils/api';
import * as dapi from '@/utils/doser_api';
import type { DropdownMenuItem } from '@nuxt/ui';
import type { MotorConfig } from '~~/shared/types/doser';

const doser = defineModel<DoserInfo>('doser', { required: true });
const motor = defineModel<MotorConfig>({ required: true });
const wakelock = reactive(useWakeLock());

const calibration_modal = ref(false);
const prime_extra_modal = ref(false);
const prime_extra_ml = ref(0.0);
const settings = ref<DropdownMenuItem[]>([
  {
    label: 'Calibrate',
    icon: 'i-tabler-droplet-cog',
    onSelect: (_) => (calibration_modal.value = true),
  },
  {
    label: 'Prime extra amount',
    icon: 'i-tabler-droplet-plus',
    onSelect: (_) => (prime_extra_modal.value = true),
  },
]);

async function prime_one(m: MotorConfig) {
  await wakelock.request('screen');
  await api.add_doser(doser.value);
  await dapi.dispense(doser.value.url, {
    reqs: [
      {
        motor_idx: m.idx,
        ml: m.prime_ml!,
      },
    ],
  });
  await wakelock.release();
}

async function prime_extra(m: MotorConfig, extra_ml: number) {
  m.prime_ml! += extra_ml; // TODO: keep value in range

  await wakelock.request('screen');
  await api.add_doser(doser.value);
  await dapi.dispense(doser.value.url, {
    reqs: [
      {
        motor_idx: m.idx,
        ml: extra_ml,
      },
    ],
  });
  await wakelock.release();
  prime_extra_modal.value = false;
}
</script>

<template>
  <UCard variant="outline">
    <UContainer class="flex flex-col items-center justify-center">
      <h2 class="font-bold text-lg">{{ motor.name }}</h2>
    </UContainer>
    <USeparator
      class="h-8 mb-2"
      :label="`Motor #${motor.idx}`"
      :ui="{ label: 'text-(--ui-color-neutral-500)' }"
    />
    <UButtonGroup>
      <UButton
        :label="`Prime ${motor.prime_ml}mL`"
        icon="i-lucide-droplet"
        color="neutral"
        variant="soft"
        class="w-full min-w-34"
        loading-auto
        @click="prime_one(motor)"
      />
      <UPopover
        arrow
        :content="{
          align: 'center',
          side: 'bottom',
        }"
      >
        <UButton icon="i-lucide-chevron-down" color="neutral" variant="subtle" />
        <template #content>
          <div class="flex flex-col items-center min-w-48 p-4 gap-4">
            <UInputNumber v-model="motor.prime_ml" :min="0" :max="20" :step="0.1" />
            <USlider v-model="motor.prime_ml" :min="0" :max="20" :step="0.1" />
          </div>
        </template>
      </UPopover>
      <UDropdownMenu arrow :items="settings">
        <UButton icon="i-lucide-settings" color="neutral" variant="subtle" />
      </UDropdownMenu>
    </UButtonGroup>
  </UCard>
  <UModal v-model:open="calibration_modal" :dismissible="false" title="Motor Calibration">
    <template #body>
      <MotorCalibration v-model="motor" v-model:doser="doser" v-model:modal="calibration_modal" />
    </template>
  </UModal>
  <UModal v-model:open="prime_extra_modal" title="Prime extra amount" class="max-w-60">
    <template #body>
      <div class="flex flex-col items-center text-center w-full gap-4">
        <UInputNumber v-model="prime_extra_ml" :min="0" :max="20 - motor.prime_ml!" :step="0.1" />
        <UButton
          label="Prime"
          leading-icon="i-tabler-droplet-plus"
          :block="true"
          loading-auto
          @click="prime_extra(motor, prime_extra_ml)"
        />
      </div>
    </template>
  </UModal>
</template>

<script setup lang="ts">
import * as api from '@/utils/api';
import * as dapi from '@/utils/doser_api';
import type { DropdownMenuItem } from '@nuxt/ui';
import type { MotorConfig } from '~~/shared/types/doser';

const doser = defineModel<DoserInfo>('doser', { required: true });
const motor = defineModel<MotorConfig>({ required: true });

const calibration_modal = ref(false);
const settings = ref<DropdownMenuItem[]>([
  {
    label: 'Calibrate',
    icon: 'i-tabler-droplet-cog',
    onSelect: (_) => (calibration_modal.value = true),
  },
]);

async function prime_one(m: MotorConfig) {
  await api.add_doser(doser.value);
  await dapi.dispense(doser.value.url, {
    reqs: [
      {
        motor_idx: m.idx,
        ml: m.prime_ml!,
      },
    ],
  });
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
      <UPopover arrow>
        <UButton icon="i-lucide-chevron-down" color="neutral" variant="subtle" />
        <template #content>
          <USlider v-model="motor.prime_ml" class="min-w-48 p-4" :min="0" :max="20" :step="0.1" />
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
</template>

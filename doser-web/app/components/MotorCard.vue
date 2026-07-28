<script setup lang="ts">
import * as api from '@/utils/api';
import * as dapi from '@/utils/doser_api';
import type { DropdownMenuItem } from '@nuxt/ui';
import type { MotorConfig } from '~~/shared/types/doser';

const doser = defineModel<DoserInfo>('doser', { required: true });
const motor = defineModel<MotorConfig>({ required: true });

const calibration_modal = ref(false);
const clear_calibration_modal = ref(false);
const dispense_modal = ref(false);
const dispense_amt = ref(1.0);
const settings = ref<DropdownMenuItem[]>([
  {
    label: 'Calibrate',
    icon: 'i-tabler-droplet-cog',
    onSelect: (_) => (calibration_modal.value = true),
  },
  {
    label: 'Clear calibration',
    icon: 'i-lucide-triangle-alert',
    onSelect: (_) => (clear_calibration_modal.value = true),
  },
  {
    label: 'Dispense',
    icon: 'i-lucide-droplets',
    onSelect: (_) => (dispense_modal.value = true),
  },
]);

async function clear_calibration() {
  await dapi.clear_calibration(doser.value.url, { motor_idx: motor.value.idx });
  clear_calibration_modal.value = false;
}
</script>

<template>
  <UCard variant="outline" class="w-9/10">
    <UContainer class="flex flex-col items-center justify-center">
      <h2 class="font-bold text-lg">{{ motor.name }}</h2>
    </UContainer>
    <USeparator
      class="h-8 mb-2"
      :label="`Motor #${motor.idx}`"
      :ui="{ label: 'text-(--ui-color-neutral-500)' }"
    />
    <UFieldGroup class="w-full">
      <UButton
        label="Unprime"
        icon="i-lucide-droplet-off"
        color="neutral"
        variant="subtle"
        class="w-full"
        loading-auto
        @click="dapi.unprime(doser.url, { motor_idx: motor.idx })"
      />
      <UButton
        label="Prime"
        icon="i-lucide-droplet"
        color="neutral"
        variant="subtle"
        class="w-full"
        loading-auto
        @click="dapi.dispense(doser.url, {reqs:[{ motor_idx: motor.idx, ml: 0 }]})"
      />
      <UDropdownMenu arrow :items="settings">
        <UButton icon="i-lucide-settings" color="neutral" variant="subtle" />
      </UDropdownMenu>
    </UFieldGroup>
  </UCard>
  <UModal v-model:open="calibration_modal" :dismissible="false" title="Motor Calibration">
    <template #body>
      <MotorCalibration v-model="motor" v-model:doser="doser" v-model:modal="calibration_modal" />
    </template>
  </UModal>
  <UModal v-model:open="clear_calibration_modal" title="Clear Calibration">
    <template #body>
      <div class="flex flex-col gap-4">
        <p>
          Clear the calibration for {{ motor.name }}? You will need to calibrate this motor again before
          dosing accurately.
        </p>
        <div class="flex justify-end gap-2">
          <UButton label="Cancel" color="neutral" variant="subtle" @click="clear_calibration_modal = false" />
          <UButton
            label="Clear calibration"
            color="error"
            icon="i-lucide-triangle-alert"
            loading-auto
            @click="clear_calibration"
          />
        </div>
      </div>
    </template>
  </UModal>
  <UModal v-model:open="dispense_modal" :dismissible="false" :title="`Dispense ${motor.name}`">
    <template #body>
      <div class="flex flex-col items-center text-center w-full gap-4">
        <UFieldGroup size="xl">
          <UInputNumber
            v-model="dispense_amt"
            :min="0.0"
            :step="1.0"
            :step-snapping="false"
            :ui="{ base: ['text-4xl'] }"
          />
          <UBadge label="mL" variant="subtle" color="neutral" />
        </UFieldGroup>
        <UButton
          label="Dispense"
          leading-icon="i-lucide-droplets"
          :block="true"
          loading-auto
          size="xl"
          @click="dapi.dispense(doser.url, { reqs: [{ motor_idx: motor.idx, ml: dispense_amt }] })"
        />
      </div>
    </template>
  </UModal>
</template>

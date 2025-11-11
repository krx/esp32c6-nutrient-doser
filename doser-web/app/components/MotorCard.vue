<script setup lang="ts">
import * as api from '@/utils/api';
import * as dapi from '@/utils/doser_api';
import type { DropdownMenuItem } from '@nuxt/ui';
import type { MotorConfig } from '~~/shared/types/doser';

const doser = defineModel<DoserInfo>('doser', { required: true });
const motor = defineModel<MotorConfig>({ required: true });

const calibration_modal = ref(false);
const priming_modal = ref(false);
const dispense_modal = ref(false);
const dispense_amt = ref(1.0);
const settings = ref<DropdownMenuItem[]>([
  {
    label: 'Calibrate',
    icon: 'i-tabler-droplet-cog',
    onSelect: (_) => (calibration_modal.value = true),
  },
  {
    label: 'Priming settings',
    icon: 'i-tabler-droplet-plus',
    onSelect: (_) => (priming_modal.value = true),
  },
  {
    label: 'Dispense',
    icon: 'i-lucide-droplets',
    onSelect: (_) => (dispense_modal.value = true),
  },
]);

function add_priming_steps(steps: number) {
  motor.value.prime_steps = Math.max(0, motor.value.prime_steps! + steps);
}

async function set_priming_steps() {
  await api.add_doser(doser.value);
  await dapi.update_prime(doser.value.url, {
    motor_idx: motor.value.idx,
    prime_steps: motor.value.prime_steps!,
  });
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
        variant="soft"
        class="w-full"
        loading-auto
        @click="dapi.unprime(doser.url, { motor_idx: motor.idx })"
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
  <UModal v-model:open="priming_modal" title="Priming Setup">
    <template #body>
      <div class="flex flex-col items-center text-center w-full gap-4">
        <UFieldGroup>
          <UInputNumber v-model="motor.prime_steps" :min="0" :step="1" />
          <UBadge label="steps" variant="subtle" color="neutral" />
        </UFieldGroup>
        <div class="flex flex-row items-center text-center gap-2">
          <UButton label="-1000" @click="add_priming_steps(-1000)" />
          <UButton label="-100" @click="add_priming_steps(-100)" />
          <UButton label="-10" @click="add_priming_steps(-10)" />
          <UButton label="+10" @click="add_priming_steps(10)" />
          <UButton label="+100" @click="add_priming_steps(100)" />
          <UButton label="+1000" @click="add_priming_steps(1000)" />
        </div>
        <UButton
          label="Set"
          leading-icon="i-tabler-droplet-plus"
          :block="true"
          loading-auto
          @click="set_priming_steps"
        />
      </div>
    </template>
  </UModal>
  <UModal v-model:open="dispense_modal" :dismissible="false" :title="`Dispense ${motor.name}`">
    <template #body>
      <div class="flex flex-col items-center text-center w-full gap-4">
        <UFieldGroup>
          <UInputNumber v-model="dispense_amt" :min="0.0" :step="1.0" :step-snapping="false" />
          <UBadge label="mL" variant="subtle" color="neutral" />
        </UFieldGroup>
        <UButton
          label="Dispense"
          leading-icon="i-lucide-droplets"
          :block="true"
          loading-auto
          @click="dapi.dispense(doser.url, { reqs: [{ motor_idx: motor.idx, ml: dispense_amt }] })"
        />
      </div>
    </template>
  </UModal>
</template>

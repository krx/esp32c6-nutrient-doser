<script setup lang="ts">
import * as api from '@/utils/api';
import * as dapi from '@/utils/doser_api';
import * as v from 'valibot';
import type { DoserInfo, MotorConfig } from '~~/shared/types/doser';
import _ from 'lodash';
import type { Chart } from '~/types/feedchart';
import type { StepperItem } from '@nuxt/ui';

const schema = v.object({
  hostname: v.pipe(v.string(), v.url('Enter a valid URL')),
});

const state = reactive({
  hostname: '',
});

const dosers = defineModel<DoserInfo[]>({ required: true });
const modal_open = defineModel<boolean>('modal', { default: false });
const all_charts = defineModel<{ [key: string]: Chart }>('all-charts', { required: true });

const selected_chart = ref('');
const motors = ref<MotorConfig[]>([]);

const stepper = useTemplateRef('stepper');
const active_step = ref(0);
const steps = ref<StepperItem[]>([
  {
    slot: 'connect',
    title: 'Connect',
    description: 'to a new doser',
  },
  {
    slot: 'config',
    title: 'Configure',
    description: 'Map motors to nutrients',
  },
]);

async function try_connect() {
  try {
    const info = await dapi.get_info(state.hostname);
    motors.value = info.motors!.map<MotorConfig>((m) => {
      return { idx: m.idx, prime_steps: m.prime_steps };
    });
    stepper.value?.next();
  } catch (err) {
    console.log(`Failed to connect: ${err}`);
  }
}

async function submit() {
  const doser: DoserInfo = {
    url: state.hostname,
    motors: motors.value,
    chart: selected_chart.value,
  };
  api.add_doser(doser).then(() => {
    _.remove(dosers.value, (d) => d.url === doser.url);
    dosers.value.push(doser);
    modal_open.value = false;
  });
}
</script>

<template>
  <div class="w-full">
    <UStepper ref="stepper" v-model="active_step" :items="steps" disabled class="w-full">
      <template #connect>
        <div class="flex flex-col justify-center items-center gap-4">
          <UForm :schema="schema" :state="state" class="w-full" @submit="try_connect">
            <UFormField label="Device URL" name="hostname" required>
              <UInput
                v-model="state.hostname"
                type="url"
                placeholder="device IP/hostname"
                trim
                class="w-full"
              />
            </UFormField>
            <USeparator class="h-4" />
            <div class="flex flex-row w-full">
              <UButton
                class="ml-auto"
                type="submit"
                label="Next"
                trailing-icon="i-lucide-arrow-right"
                loading-auto
              />
            </div>
          </UForm>
        </div>
      </template>
      <template #config>
        <div class="flex flex-col justify-center items-center gap-4">
          <USelect
            v-model="selected_chart"
            :items="Object.values(all_charts).map((c) => c.name)"
            class="w-full"
            placeholder="Select a feedchart"
          />
          <div v-if="selected_chart" class="flex flex-col w-full gap-2">
            <USeparator class="h-1 mb-2" />
            <UFieldGroup v-for="motor in motors" :key="motor.idx">
              <UBadge
                :label="`Motor #${motor.idx}`"
                class="w-22"
                variant="subtle"
                color="neutral"
              />
              <USelect
                v-model="motor.name"
                :items="
                  all_charts[selected_chart]?.nutrients
                    .filter((n) => n.unit == 'mL')
                    .map((n) => n.name)
                "
                class="w-full"
              />
            </UFieldGroup>
          </div>
          <USeparator class="h-4" />
          <div class="flex flex-row w-full">
            <UButton label="Back" icon="i-lucide-arrow-left" @click="stepper?.prev()" />
            <UButton
              class="ml-auto"
              label="Done"
              trailing-icon="i-lucide-arrow-right"
              loading-auto
              :disabled="!motors.every((m) => m.name)"
              @click="submit"
            />
          </div>
        </div>
      </template>
    </UStepper>
  </div>
</template>

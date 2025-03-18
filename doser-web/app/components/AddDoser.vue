<script setup lang="ts">
import * as api from '@/utils/api';
import * as v from 'valibot';
import type { DoserInfo } from '~~/shared/types/doser';
import _ from 'lodash';

const schema = v.object({
  hostname: v.pipe(v.string(), v.url('Enter a valid URL')),
});

const state = reactive({
  hostname: '',
});

const dosers = defineModel<DoserInfo[]>({ required: true });
const modal_open = defineModel<boolean>('modal', { default: false });

async function submit() {
  const doser: DoserInfo = { url: state.hostname, motors: [] };
  api.add_doser(doser).then(() => {
    _.remove(dosers.value, (d) => d.url === doser.url);
    dosers.value.push(doser);
    modal_open.value = false;
  });
}
</script>

<template>
  <UForm :schema="v.safeParser(schema)" :state="state" @submit="submit">
    <UFormField label="Device URL" name="hostname" required>
      <UButtonGroup class="w-full">
        <UInput v-model="state.hostname" type="url" placeholder="device IP/hostname" trim />
        <UButton type="submit" label="Add" />
      </UButtonGroup>
    </UFormField>
  </UForm>
</template>

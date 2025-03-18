<script setup lang="ts">
import * as api from '@/utils/api';
import _ from 'lodash';
import type { DoserInfo } from '~~/shared/types/doser';

const all_dosers = defineModel<DoserInfo[]>('dosers', { required: true });
const selected = defineModel<DoserInfo | undefined>('selected', { required: true });
const _sel = ref('');
const add_modal_opened = ref(false);

function remove_doser() {
  api.remove_doser(_sel.value);
  _.remove(all_dosers.value, (d) => d.url === _sel.value);

  selected.value = undefined;
  _sel.value = '';
}
</script>

<template>
  <UButtonGroup>
    <UModal v-model:open="add_modal_opened" title="Add new doser">
      <UButton icon="lucide:plus" label="New" />
      <template #body>
        <div class="flex flex-col items-center justify-center py-12">
          <AddDoser v-model="all_dosers" v-model:modal="add_modal_opened" />
        </div>
      </template>
    </UModal>
    <USelect
      v-model="_sel"
      :items="all_dosers.map((d) => d.url)"
      class="min-w-64"
      @change="selected = all_dosers.find((d) => d.url === _sel)"
    />
    <UButton :disabled="_sel === ''" color="error" icon="lucide:trash-2" @click="remove_doser" />
  </UButtonGroup>
</template>

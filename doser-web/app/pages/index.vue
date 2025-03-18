<script setup lang="ts">
import * as api from '@/utils/api';
import type { DoserInfo } from '#shared/types/doser';

const dosers = ref<DoserInfo[]>([]);
const selected = ref<DoserInfo | undefined>();

(async () => {
  // get configured doser endpoints
  dosers.value = await api.get_dosers();
})();
</script>

<template>
  <UContainer class="flex flex-col items-center justify-center gap-4 overflow-y-auto my-8 max-w-2xl">
    <h1 class="font-bold text-2xl text-(--ui-primary)">Nutrient Doser</h1>

    <div v-show="$pwa?.needRefresh">
      <span> New content available, click on reload button to update. </span>

      <button @click="$pwa?.updateServiceWorker()">Reload</button>
    </div>

    <AddDoser v-if="dosers.length === 0" v-model="dosers" />
    <SelectDoser v-else v-model:dosers="dosers" v-model:selected="selected" />

    <DoserView v-if="selected !== undefined" :key="selected.url" v-model="selected" />
  </UContainer>
</template>

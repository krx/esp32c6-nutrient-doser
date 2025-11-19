<script setup lang="ts">
import type { Chart } from '~/types/feedchart';
import DoserApp from '~/components/root/DoserApp.vue';
import type { TabsItem } from '@nuxt/ui';
import CalcApp from '~/components/root/CalcApp.vue';

const all_charts: { [key: string]: Chart } = [
  (await import('@/assets/floraseries.json')) as Chart,
  (await import('@/assets/floranova.json')) as Chart,
  (await import('@/assets/maxiseries_indoor.json')) as Chart,
  (await import('@/assets/maxiseries_outdoor.json')) as Chart,
].reduce((obj, c) => ({ ...obj, [c.name]: c }), {});

const tabs = ref<TabsItem[]>([
  {
    label: 'Doser',
    slot: 'doser',
  },
  {
    label: 'Calculator',
    slot: 'calculator',
  },
]);
</script>

<template>
  <div>
    <UModal :open="$pwa?.needRefresh" class="flex flex-col" :dismissible="false">
      <template #body>
        New content available, click on reload button to update.
        <UButton
          label="Reload"
          icon="i-lucide-refresh"
          :block="true"
          @click="$pwa?.updateServiceWorker()"
        />
      </template>
    </UModal>

    <UTabs :items="tabs" variant="link" :unmount-on-hide="false" size="xl">
      <template #doser>
        <DoserApp :all-charts="all_charts" />
      </template>
      <template #calculator>
        <CalcApp :all-charts="all_charts" />
      </template>
    </UTabs>
  </div>
</template>

<script setup lang="ts">
import type { TabsItem } from '@nuxt/ui';

const selected = defineModel<string>({ required: true });
const items = defineModel<TabsItem[]>('items', { required: true });
const ui = defineModel<object>('ui', { required: false });
</script>

<template>
  <UTabs v-model="selected" class="w-full" size="xl" :items="items" :content="false" :ui="ui" />
</template>

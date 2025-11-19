<script setup lang="ts">
import type { Chart, GrowthStage, VolUnit } from '@/types/feedchart';
import { PieChart } from 'echarts/charts';
import { LegendComponent, TitleComponent, TooltipComponent } from 'echarts/components';
import { use } from 'echarts/core';
import { CanvasRenderer } from 'echarts/renderers';
import _ from 'lodash';
import VChart, { THEME_KEY } from 'vue-echarts';
import defaultdict from 'defaultdict-proxy';
import type { NutrientTable } from '~/types/nutrients';
import type { EChartsOption } from 'echarts';
import type { PieDataItemOption } from 'echarts/types/src/chart/pie/PieSeries.js';

use([CanvasRenderer, PieChart, TitleComponent, TooltipComponent, LegendComponent]);

provide(THEME_KEY, 'dark');

const all_nutrients: NutrientTable = (await import('@/assets/nutrients.json')).default as NutrientTable;

const target_amount = defineModel<number>('amount', { required: true });
const target_unit = defineModel<VolUnit>('unit', { required: true });
const chart = defineModel<Chart>('chart', { required: true });
const stage = defineModel<GrowthStage>('stage', { required: true });

function compute_nutrients(value_label = false) {
  const target_gal = to_gal(target_amount.value, target_unit.value);

  const res: Array<PieDataItemOption> = [];
  chart.value.nutrients.forEach((n) => {
    const amt = (stage.value[n.name] || 0) * target_gal;
    if (amt > 0) {
      res.push({
        name: n.name,
        value: amt,
        itemStyle: {
          color: n.color,
        },
        label: {
          formatter: (args) => value_label ? `${_.round(args.value, 3)} ${n.unit}` : `${args.name}`,
        },
        tooltip: {
          valueFormatter: (val: number) => `${_.round(val, 3)} ${n.unit}`,
        },
      });
    }
  });
  return res;
}

function compute_micros() {
  const micros = defaultdict(0.0);
  const names = {};
  let total = 0.0;
  compute_nutrients().forEach((n) => {
    total += n.value;
    all_nutrients
      .find((v) => v.name === n.name)
      ?.nutrients.forEach((micro) => {
        micros[micro.abbr] += n.value * micro.pcnt / 100.0;
        names[micro.abbr] = micro.name;
      });
  });

  return Object.keys(micros).map((m) => {
    return {
      name: m,
      value: micros[m],
      label: {
        formatter: '{b}',
      },
      tooltip: {
        formatter: `${names[m]}: <b>${_.round(micros[m] * 1000, 2)}mg (${_.round((micros[m] / total) * 100.0, 2)}%)</b>`,
      },
    };
  });
}

const option = ref<EChartsOption>({
  backgroundColor: 'transparent',
  title: {
    text: 'Amounts to Mix',
    left: 'center',
    top: 0,
  },
  tooltip: {
    trigger: 'item',
  },
  legend: {
    orient: 'horizontal',
    left: 'center',
    top: 'bottom',
  },
  series: [
    {
      type: 'pie',
      radius: ['40%', '90%'],
      data: computed(() => compute_nutrients(true)),
      label: {
        show: true,
        position: 'inside',
        fontSize: 17,
        fontWeight: 'bold',
      },
    },
  ],
});

const option_micros = ref<EChartsOption>({
  backgroundColor: 'transparent',
  title: {
    text: 'Nutrient Analysis',
    left: 'center',
    top: 0,
  },
  tooltip: {
    trigger: 'item',
  },
  legend: {
    orient: 'horizontal',
    type: 'scroll',
    left: 'center',
    top: 'bottom',
  },
  series: [
    {
      type: 'pie',
      radius: '70%',
      data: computed(() => compute_micros()),
    },
  ],
});
</script>

<template>
  <UCard variant="subtle">
    <UContainer class="flex flex-col gap-2 items-center justify-center h-128">
      <VChart :option="option" :autoresize="true" />
    </UContainer>
  </UCard>
  <UCard variant="subtle">
    <UContainer class="flex flex-col gap-2 items-center justify-center h-128">
      <VChart :option="option_micros" :autoresize="true" />
    </UContainer>
  </UCard>
</template>

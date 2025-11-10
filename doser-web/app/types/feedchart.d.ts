export type VolUnit = "mL" | "L" | "fl oz" | "gal";
export type NutrientUnit = "mL" | "g";

export interface Nutrient {
  name: string;
  unit: NutrientUnit;
  color: string;
}

export interface GrowthStage {
  [nutrient: string]: number;
}

export interface FeedSchedule {
  [stage: string]: GrowthStage;
}

export interface FeedChart {
  [schedule: string]: FeedSchedule;
}

export interface Chart {
  name: string;
  nutrients: Nutrient[];
  charts: FeedChart;
}

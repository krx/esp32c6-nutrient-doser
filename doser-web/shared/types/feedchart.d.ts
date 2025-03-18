export type Nutrient = string;

/**
 * Amount of nutrient to use in ml/gal
 */
export type NutrientAmount = number;

export interface GrowthStage {
  [nutrient: Nutrient]: NutrientAmount;
}

export interface FeedSchedule {
  [stage: string]: GrowthStage;
}

export interface FeedChart {
  [schedule: string]: FeedSchedule;
}

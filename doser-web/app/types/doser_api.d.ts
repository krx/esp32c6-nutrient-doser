import type { Nutrient, NutrientAmount } from "#shared/types/feedchart";

export interface InfoResp {
  num_motors: number;
}

export interface Dispense {
  motor_idx: number;
  ml: number;
}

export interface DispenseReq {
  reqs: Dispense[];
}

export interface CalibrateReq {
  motor_idx: number;
  expected: number;
  actual: number;
}

export type VolUnit = 'mL' | 'Ml' | 'L' | 'gal' | 'Gal';

export interface NutrientInfo {
  name: Nutrient;
  motor_idx: number;
  ml_per_gal: NutrientAmount;
}

export interface DoseSolutionReq {
  nutrients: NutrientInfo[];
  target_amount: number;
  target_unit: VolUnit;
}

export interface OtaReq {
  uri: URL;
}

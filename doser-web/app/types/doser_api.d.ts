import type { VolUnit } from '#/types/feedchart';

export enum Status {
  IDLE,
  DISPENSING,
  OTA,
}

export interface MotorStatus {
  idx: number;
  id: number;
  position: number;
  is_primed: boolean;
  prime_steps: number;
  ml_per_step: number;
}

export interface StatusResp {
  num_motors?: number;
  motors?: MotorStatus[];
  version?: string;
  status: Status;
}

export interface Dispense {
  motor_idx: number;
  ml: number;
}

export interface DispenseReq {
  reqs: Dispense[];
}

export interface DebugStepReq {
  motor_idx: number;
  steps: number;
}

export interface UpdatePrimeReq {
  motor_idx: number;
  prime_steps: number;
}

export interface UnprimeReq {
  motor_idx: number;
}

export interface CalibrateReq {
  motor_idx: number;
  expected: number;
  actual: number;
}

export interface NutrientInfo {
  name: string;
  motor_idx: number;
  ml_per_gal: number;
}

export interface DoseSolutionReq {
  nutrients: NutrientInfo[];
  target_amount: number;
  target_unit: VolUnit;
}

export interface OtaReq {
  uri: URL;
}

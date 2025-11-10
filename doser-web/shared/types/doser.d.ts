export interface MotorConfig {
  idx: number;
  name?: string;
  prime_steps?: number;
}

export interface DoserInfo {
  url: string;
  motors: MotorConfig[];
  chart: string;
}

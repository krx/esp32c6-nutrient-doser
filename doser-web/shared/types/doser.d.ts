export interface MotorConfig {
  idx: number;
  name?: string;
  prime_ml?: number;
  // TODO: image?
}

export interface DoserInfo {
  // id: number;
  url: string;
  motors: MotorConfig[];
}

// export type DoserMap = Map<string, DoserInfo>;

export type NutrientUnit = "mL" | "g";

export interface Nutrient {
  name: string;
  unit: NutrientUnit;
  color: string;
  density?: number;
  nutrients: Chemical[];
}

export interface Chemical {
  name: string;
  abbr: string;
  pcnt: number;
}

export type NutrientTable = Nutrient[];

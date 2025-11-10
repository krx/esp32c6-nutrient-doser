import type { VolUnit } from '~/types/feedchart';

export function scale_to_ml(unit: VolUnit): number {
  switch (unit) {
    case 'mL':    return 1.0;
    case 'L':     return 1000.0;
    case 'fl oz': return 29.5735;
    case 'gal':   return 3785.41;
  }
}

export function to_ml(val: number, unit: VolUnit) {
  return val * scale_to_ml(unit);
}

export function to_gal(val: number, unit: VolUnit) {
  return to_ml(val, unit) / scale_to_ml('gal');
}

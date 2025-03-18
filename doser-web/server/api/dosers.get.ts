import type { DoserInfo } from '~~/shared/types/doser';
import { load_all_dosers } from './common';

export default defineEventHandler(async (): Promise<DoserInfo[]> => {
  return await load_all_dosers();
});

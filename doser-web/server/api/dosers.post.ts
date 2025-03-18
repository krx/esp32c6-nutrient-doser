import { save_doser } from './common';

export default defineEventHandler(async (event) => {
  readBody<DoserInfo>(event).then(async (res) => {
    await save_doser(res);
  });
});

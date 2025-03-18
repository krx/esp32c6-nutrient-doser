import { delete_doser } from './common';

export default defineEventHandler(async (event) => {
  readBody<{ hostname: URL }>(event).then(async (res) => {
    await delete_doser(res.hostname);
  });
});

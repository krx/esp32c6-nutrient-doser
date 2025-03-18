import type { CalibrateReq, DispenseReq, DoseSolutionReq, InfoResp } from '~/types/doser_api';

export async function get_info(url_base: string) {
  return await $fetch<InfoResp>(`${url_base}/info`);
}

export async function dispense(url_base: string, req: DispenseReq) {
  return await $fetch(`${url_base}/dispense`, {
    method: 'POST',
    body: req,
  });
}

export async function calibrate(url_base: string, req: CalibrateReq) {
  return await $fetch(`${url_base}/calibrate`, {
    method: 'POST',
    body: req,
  });
}

export async function dose_solution(url_base: string, req: DoseSolutionReq) {
  return await $fetch(`${url_base}/dose`, {
    method: 'POST',
    body: req,
  });
}

import type {
  CalibrateReq,
  DebugStepReq,
  DispenseReq,
  DoseSolutionReq,
  OtaReq,
  StatusResp,
  UnprimeReq,
  UpdatePrimeReq,
} from '~/types/doser_api';

export async function get_info(url_base: string) {
  return await $fetch<StatusResp>(`${url_base}/full-status`, {
    timeout: 2000,
  });
}

export async function status(url_base: string) {
  return (await $fetch<StatusResp>(`${url_base}/status`)).status;
}

export function dispense(url_base: string, req: DispenseReq) {
  return $fetch(`${url_base}/dispense`, {
    method: 'POST',
    body: req,
  });
}

export function debug_step(url_base: string, req: DebugStepReq) {
  return $fetch(`${url_base}/debug-step`, {
    method: 'POST',
    body: req,
  });
}

export async function update_prime(url_base: string, req: UpdatePrimeReq) {
  return await $fetch(`${url_base}/update-prime`, {
    method: 'POST',
    body: req,
  });
}

export async function unprime(url_base: string, req: UnprimeReq) {
  return await $fetch(`${url_base}/unprime`, {
    method: 'POST',
    body: req,
  });
}

export function unprime_all(url_base: string) {
  return $fetch(`${url_base}/unprime-all`, {
    method: 'POST',
  });
}

export async function calibrate(url_base: string, req: CalibrateReq) {
  return await $fetch(`${url_base}/calibrate`, {
    method: 'POST',
    body: req,
  });
}

export function dose_solution(url_base: string, req: DoseSolutionReq) {
  return $fetch(`${url_base}/dose`, {
    method: 'POST',
    body: req,
  });
}

export async function reboot(url_base: string) {
  return await $fetch(`${url_base}/reboot`);
}

export async function ota(url_base: string, req: OtaReq) {
  return await $fetch(`${url_base}/ota`, {
    method: 'POST',
    body: req,
  });
}

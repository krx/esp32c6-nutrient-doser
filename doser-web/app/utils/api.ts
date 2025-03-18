import type { DoserInfo } from '~~/shared/types/doser';

// TODO: dynamically generate this base from host this is running on
// export const API_BASE = 'http://192.168.10.27:8000'

export async function get_dosers(): Promise<DoserInfo[]> {
  return await $fetch<DoserInfo[]>('/api/dosers');
}

export async function add_doser(d: DoserInfo) {
  await $fetch('/api/dosers', {
    method: 'POST',
    body: d,
  });
}

export async function remove_doser(host: string) {
  await $fetch('/api/dosers', {
    method: 'DELETE',
    body: { hostname: host },
  });
}

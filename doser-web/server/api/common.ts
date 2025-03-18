import type { DoserInfo } from '~~/shared/types/doser';

export const DOSER_BASE = 'dosers:';

export function get_storage() {
  return useStorage('data');
}

export async function load_all_dosers(): Promise<DoserInfo[]> {
  const store = get_storage();
  return Promise.all(
    (await store.getKeys(DOSER_BASE)).map(
      async (key) => (await store.getItem<DoserInfo>(key)) as DoserInfo,
    ),
  );
}

export async function load_doser(url: URL): Promise<DoserInfo> {
  return (await get_storage().getItem(`${DOSER_BASE}${url.toString()}`)) as DoserInfo;
}

export async function save_doser(d: DoserInfo): Promise<void> {
  await get_storage().setItem(`${DOSER_BASE}${d.url.toString()}`, d);
}

export async function delete_doser(url: URL): Promise<void> {
  await get_storage().removeItem(`${DOSER_BASE}${url.toString()}`);
}

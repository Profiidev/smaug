import type { PageLoad } from './$types';
import type { EggListItem } from './table.svelte';

export const load: PageLoad = ({ url }) => {
  // TODO: replace with listEggs({ fetch }).then(({ data }) => data?.eggs ?? []) from $lib/client
  // once backend egg routes exist.
  const eggs: Promise<EggListItem[]> = Promise.resolve([]);

  return {
    eggs,
    error: url.searchParams.get('error')
  };
};

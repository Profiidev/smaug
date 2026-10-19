import { listEggs } from '$lib/client';
import type { PageLoad } from './$types';

export const load = (({ fetch, url }) => ({
  eggs: listEggs({ fetch }).then((r) => r.data?.eggs),
  error: url.searchParams.get('error')
})) satisfies PageLoad;

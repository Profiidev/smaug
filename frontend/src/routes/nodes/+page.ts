import { listNodes } from '#lib/client/index.ts';
import type { PageLoad } from './$types';

export const load: PageLoad = ({ fetch, url }) => ({
  error: url.searchParams.get('error'),
  nodes: listNodes({ fetch }).then((r) => r.data)
});

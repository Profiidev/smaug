import { serverOptions } from '$lib/client';
import type { PageLoad } from './$types';

export const load = (({ fetch }) => ({
  options: serverOptions({ fetch }).then((r) => r.data)
})) satisfies PageLoad;

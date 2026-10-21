import { serverInfo, serverOptions } from '$lib/client';
import type { PageLoad } from './$types';

export const load = (({ fetch, params }) => {
  const serverRes = serverInfo({ fetch, path: { uuid: params.uuid } });
  return {
    // Only admins may load (and need) the owner / egg / node / grant lists.
    options: serverRes.then(async (r) =>
      r.data?.access === 'admin'
        ? (await serverOptions({ fetch })).data
        : undefined
    ),
    serverRes,
    uuid: params.uuid
  };
}) satisfies PageLoad;

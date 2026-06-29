import type { LayoutLoad } from './$types';
import { nodeInfo } from '$lib/client';

export const load: LayoutLoad = ({ params, fetch }) => ({
  nodeRes: nodeInfo({
    fetch,
    path: {
      uuid: params.uuid
    }
  }),
  uuid: params.uuid
});

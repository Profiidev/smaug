import type { LayoutLoad } from './$types';
import { nodeInfo } from '$lib/client';

export const load: LayoutLoad = ({ params, fetch }) => {
  return {
    nodeRes: nodeInfo({
      path: {
        uuid: params.uuid
      },
      fetch
    }),
    uuid: params.uuid
  };
};

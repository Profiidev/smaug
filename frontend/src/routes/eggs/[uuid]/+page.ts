import type { PageLoad } from './$types';
import type { EggJson } from '../schema.svelte';

export const load: PageLoad = ({ params }) => {
  // TODO: replace with eggInfo({ fetch, path: { uuid: params.uuid } }) from $lib/client
  // once backend egg routes exist. Resolving to a 404-shaped result for now so the
  // page's existing not-found redirect fires, matching real backend behavior.
  const eggRes: Promise<{ data?: EggJson; response: { status: number } }> = Promise.resolve({
    data: undefined,
    response: { status: 404 }
  });

  return {
    eggRes,
    uuid: params.uuid
  };
};

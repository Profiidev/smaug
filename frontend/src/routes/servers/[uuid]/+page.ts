import { redirect } from '@sveltejs/kit';
import type { PageLoad } from './$types';

// The URL always names the open tab.
export const load = (({ params }) => {
  redirect(307, `/servers/${params.uuid}/console`);
}) satisfies PageLoad;

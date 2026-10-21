import { redirect } from '@sveltejs/kit';
import type { PageLoad } from './$types';

// Old sub-route; the node page now has tabs.
export const load = (({ params }) => {
  redirect(307, `/nodes/${params.uuid}?tab=setup`);
}) satisfies PageLoad;

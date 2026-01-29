import { redirect } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = ({ params }) => {
  redirect(302, `/nodes/${params.uuid}/setup`);
};

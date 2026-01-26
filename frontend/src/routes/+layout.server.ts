import { redirect } from '@sveltejs/kit';
import type { LayoutServerLoad } from './$types.js';

export const load: LayoutServerLoad = ({ cookies, url }) => {
  let cookie = cookies.get('smaug_jwt');

  if (!cookie && url.pathname !== '/login') {
    redirect(302, '/login');
  }
};

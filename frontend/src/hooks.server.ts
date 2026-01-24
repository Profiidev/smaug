import type { HandleFetch } from '@sveltejs/kit';
import { BACKEND_URL } from '$env/static/private';

const backendUrl = new URL(BACKEND_URL);

export const handleFetch: HandleFetch = async ({ request, fetch }) => {
  let url = new URL(request.url);
  if (url.pathname.startsWith('/api/')) {
    url.hostname = backendUrl.hostname;
    url.port = backendUrl.port;
    url.protocol = backendUrl.protocol;

    request = new Request(url.toString(), request);
  }
  return fetch(request);
};

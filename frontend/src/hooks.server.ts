import type { HandleFetch } from '@sveltejs/kit';
import { BACKEND_URL } from '$env/static/private';

const backendUrl = new URL(BACKEND_URL);

export const handleFetch: HandleFetch = async ({ event, request, fetch }) => {
  const url = new URL(request.url);
  let rewriteRequest = request;

  if (url.pathname.startsWith('/api/')) {
    url.hostname = backendUrl.hostname;
    url.port = backendUrl.port;
    url.protocol = backendUrl.protocol;

    rewriteRequest = new Request(url.toString(), rewriteRequest);

    const cookie = event.request.headers.get('cookie');
    if (cookie) {
      rewriteRequest.headers.set('cookie', cookie);
    }
  }
  return fetch(rewriteRequest);
};

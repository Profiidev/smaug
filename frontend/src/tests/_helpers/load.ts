import { vi } from 'vitest';

/** A `fetch` stub answering every request with `body` as JSON (or empty for `null`). */
export const jsonFetch = (body: unknown, status = 200) =>
  vi.fn(
    async (_input: RequestInfo | URL, _init?: RequestInit) =>
      new Response(body === null ? null : JSON.stringify(body), {
        headers: { 'Content-Type': 'application/json' },
        status
      })
  );

/** Calls a SvelteKit load with only the event fields it reads. */
export const runLoad = <T>(
  load: (event: never) => T,
  event: { fetch?: typeof fetch; url?: URL; params?: Record<string, string> }
) => load(event as never) as Awaited<T>;

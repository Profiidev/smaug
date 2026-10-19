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

/** Calls a SvelteKit load with only the event fields it reads (`void` stripped from `PageLoad` results). */
export const runLoad = <T>(
  load: (event: never) => T,
  event: { fetch?: typeof fetch; url?: URL; params?: Record<string, string> }
) => load(event as never) as Exclude<Awaited<T>, void>;

interface ThrownRedirect {
  status: number;
  location: string;
}

/** Runs `fn` and returns the SvelteKit redirect it throws. Fails if `fn` does not redirect. */
export const catchRedirect = async (
  fn: () => unknown
): Promise<ThrownRedirect> => {
  try {
    await fn();
  } catch (error) {
    // oxlint-disable-next-line no-unsafe-type-assertion
    return error as ThrownRedirect;
  }
  throw new Error('expected a redirect to be thrown');
};

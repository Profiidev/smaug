import { describe, expect, it } from 'vitest';
import { load } from '$routes/setup/+page';
import { catchRedirect, jsonFetch, runLoad } from '$test_helpers/load';

/** The setup load is async; `runLoad` types the awaited value, so resolve it. */
const runSetup = async (body: unknown, status = 200) =>
  Promise.resolve(runLoad(load, { fetch: jsonFetch(body, status) }));

describe('setup load', () => {
  it('redirects to / when setup is already complete', async () => {
    const redirect = await catchRedirect(async () =>
      runSetup({ db_backend: 'SQLite', is_setup: true })
    );
    expect(redirect.status).toBe(302);
    expect(redirect.location).toBe('/');
  });

  it('returns the db backend when setup is not complete', async () => {
    const result = await runSetup({
      db_backend: 'PostgreSQL',
      from_env: [],
      is_setup: false,
      settings: {}
    });
    expect(result.db_backend).toBe('PostgreSQL');
    expect(result.configured).toBe(false);
  });

  it('falls back to "unknown" when the requests fail', async () => {
    const result = await runSetup(null, 500);
    expect(result.db_backend).toBe('unknown');
    expect(result.configured).toBe(false);
  });
});

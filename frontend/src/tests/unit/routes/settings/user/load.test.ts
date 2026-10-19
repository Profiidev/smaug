import { describe, expect, it } from 'vitest';
import { load } from '$routes/settings/user/+page';
import { jsonFetch, runLoad } from '$test_helpers/load';

describe('settings/user load', () => {
  it('exposes the user-settings and general-settings promises', async () => {
    const fetch = jsonFetch({ oidc_enabled: true, site_url: 'https://x/' });
    const result = runLoad(load, { fetch });

    await expect(result.settingsPromise).resolves.toMatchObject({
      oidc_enabled: true
    });
    await expect(result.generalSettingsPromise).resolves.toMatchObject({
      site_url: 'https://x/'
    });
    expect(
      fetch.mock.calls.map(([req]) => new URL((req as Request).url).pathname)
    ).toEqual(['/api/settings/user', '/api/settings/general']);
  });

  it.each([401, 500])(
    'resolves both promises to undefined on %i',
    async (s) => {
      const result = runLoad(load, { fetch: jsonFetch(null, s) });
      await expect(result.settingsPromise).resolves.toBeUndefined();
      await expect(result.generalSettingsPromise).resolves.toBeUndefined();
    }
  );
});

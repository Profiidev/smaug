import { describe, expect, it } from 'vitest';
import { load as accountLoad } from '$routes/account/+page.server';
import { load as passwordLoad } from '$routes/password/+page.server';
import { load as settingsLoad } from '$routes/settings/+page.server';
import { load as nodeLoad } from '$routes/nodes/[uuid]/+page.server';
import { catchRedirect } from '$test_helpers/load';

describe('index redirects', () => {
  it.each([
    ['account', accountLoad, '/account/general'],
    ['password', passwordLoad, '/password/forgot'],
    ['settings', settingsLoad, '/settings/user']
  ])('%s redirects to %s', async (_name, load, location) => {
    const redirect = await catchRedirect(() => (load as () => unknown)());
    expect(redirect.status).toBe(302);
    expect(redirect.location).toBe(location);
  });

  it('nodes/[uuid] redirects to its setup page', async () => {
    const redirect = await catchRedirect(() =>
      (nodeLoad as (e: unknown) => unknown)({ params: { uuid: 'n1' } })
    );
    expect(redirect.status).toBe(302);
    expect(redirect.location).toBe('/nodes/n1/setup');
  });
});

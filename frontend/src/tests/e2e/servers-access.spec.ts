import { expect } from '@playwright/test';
import { HttpResponse } from 'msw';
import * as gen from '$lib/client/msw.gen';
import { test } from '$test_helpers/e2e-fixture';
import { setupSession } from '$test_helpers/session';
import { expectNoHorizontalOverflow, gotoReady } from '$test_helpers/layout';
import {
  action,
  asAccess,
  combo,
  ok,
  openTab,
  recordUpdate,
  save,
  survivalBody,
  tab
} from '$test_helpers/servers-e2e';
import { serverInfoOf, serverOptions } from '../mocks/e2e/data';

test.describe('server edit with write access', () => {
  test.beforeEach(async ({ context }) => setupSession(context));

  test('locks admin fields and saves without grants', async ({
    network,
    page
  }) => {
    let optionsCalled = false;
    const update = recordUpdate();
    network.use(
      asAccess('write'),
      update.handler,
      gen.handleServerOptions(() => {
        optionsCalled = true;
        return HttpResponse.json(serverOptions.default) as never;
      })
    );
    await gotoReady(page, '/servers/srv-survival/status');

    await expect(
      page.getByText('Some settings need an administrator')
    ).toBeVisible();
    await expect(tab(page, 'Permissions')).toHaveCount(0);
    await expect(action(page, 'Delete')).toBeDisabled();
    await expect(action(page, 'Reinstall')).toBeEnabled();
    for (const label of [
      'Mark as not installed',
      'Suspend',
      'Transfer…',
      'Delete server'
    ]) {
      // oxlint-disable-next-line no-await-in-loop
      await expect(action(page, label)).toBeDisabled();
    }

    await openTab(page, 'General');
    await expect(page.getByLabel('Name', { exact: true })).toBeEnabled();
    await expect(combo(page, 'Owner')).toBeDisabled();
    await expect(combo(page, 'Egg')).toBeDisabled();
    await expect(page.getByLabel('IP address').first()).toBeDisabled();
    await expect(
      page.getByRole('button', { name: 'Add allocation' })
    ).toBeDisabled();
    await page.getByLabel('Name', { exact: true }).fill('Writer World');

    await openTab(page, 'Startup');
    await expect(page.getByLabel('Image', { exact: true })).toBeDisabled();
    await page.getByRole('textbox', { name: 'Command' }).fill('./custom');

    await openTab(page, 'Variables');
    await expect(page.getByLabel('Server Jar File')).toBeDisabled();
    await expect(page.getByLabel('Version')).toHaveCount(0);
    await page.getByLabel('Max Players').fill('12');

    await openTab(page, 'Resources');
    await expect(
      page.getByRole('switch', { name: 'Unlimited Storage' })
    ).toBeDisabled();
    await expect(page.getByLabel('Backup limit')).toBeDisabled();
    await save(page);

    await expect(
      page.getByText('Server Writer World updated successfully')
    ).toBeVisible();
    const { grants: _grants, ...withoutGrants } = survivalBody;
    expect(update.bodies).toEqual([
      {
        ...withoutGrants,
        environment: { MAX_PLAYERS: '12' },
        name: 'Writer World',
        startup: './custom',
        startup_overridden: true
      }
    ]);
    expect(optionsCalled).toBe(false);
  });
});

test.describe('server edit with read access', () => {
  test.beforeEach(async ({ context }) => setupSession(context, 'readonly'));

  test('is fully read-only', async ({ network, page }) => {
    let optionsCalled = false;
    network.use(
      gen.handleServerOptions(() => {
        optionsCalled = true;
        return HttpResponse.json(serverOptions.default) as never;
      })
    );
    await gotoReady(page, '/servers/srv-survival/status');

    await expect(
      page.getByText('You have view-only access', { exact: true })
    ).toBeVisible();
    await expect(
      page.getByText('Actions need write access to this server.')
    ).toBeVisible();
    await expect(tab(page, 'Permissions')).toHaveCount(0);
    await expect(action(page, 'Delete')).toBeDisabled();
    for (const label of [
      'Reinstall',
      'Mark as not installed',
      'Suspend',
      'Transfer…',
      'Delete server'
    ]) {
      // oxlint-disable-next-line no-await-in-loop
      await expect(action(page, label)).toBeDisabled();
    }

    await openTab(page, 'General');
    await expect(page.getByLabel('Name', { exact: true })).toBeDisabled();
    await expect(
      page.getByRole('button', { name: 'Save Changes' })
    ).toBeDisabled();
    await openTab(page, 'Startup');
    await expect(page.getByRole('textbox', { name: 'Command' })).toBeDisabled();
    await openTab(page, 'Variables');
    await expect(page.getByLabel('Max Players')).toBeDisabled();
    await expectNoHorizontalOverflow(page);
    expect(optionsCalled).toBe(false);
  });
});

test.describe('server edit /permissions without admin access', () => {
  test.beforeEach(async ({ context }) => setupSession(context));

  for (const access of ['write', 'read'] as const) {
    test(`${access} access lands on Status`, async ({ network, page }) => {
      network.use(asAccess(access));
      await gotoReady(page, '/servers');
      await gotoReady(page, '/servers/srv-survival/permissions');

      await expect(page).toHaveURL(/\/servers\/srv-survival\/status$/);
      await expect(tab(page, 'Status')).toHaveAttribute(
        'aria-selected',
        'true'
      );
      await expect(tab(page, 'Permissions')).toHaveCount(0);
      // Replaced, not pushed: Back leaves the server page.
      await page.goBack();
      await expect(page).toHaveURL(/\/servers$/);
    });
  }
});

test.describe('server edit edge cases', () => {
  test.beforeEach(async ({ context }) => setupSession(context));

  /** Serves srv-survival with `access` and no owner (owner user deleted). */
  const ownerless = (access: 'write' | 'admin') =>
    gen.handleServerInfo(({ params }) =>
      // `/servers/options` matches this route too; leave it to its handler.
      params.uuid === 'options'
        ? undefined
        : (HttpResponse.json({
            ...serverInfoOf(params.uuid, access),
            owner: null
          }) as never)
    );

  for (const access of ['admin', 'write'] as const) {
    test(`saves a server without an owner as ${access}`, async ({
      network,
      page
    }) => {
      const update = recordUpdate();
      network.use(ownerless(access), update.handler);
      await gotoReady(page, '/servers/srv-survival/status');
      await openTab(page, 'General');
      await page.getByLabel('Name', { exact: true }).fill('Orphan');
      await save(page);

      await expect(
        page.getByText('Server Orphan updated successfully')
      ).toBeVisible();
      expect(update.bodies).toHaveLength(1);
      expect(update.bodies[0]).toMatchObject({ name: 'Orphan', owner: null });
    });
  }

  test('explains a 403 on save', async ({ network, page }) => {
    const update = recordUpdate(
      () => new HttpResponse(null, { status: 403 }) as never
    );
    network.use(update.handler);
    await gotoReady(page, '/servers/srv-survival/status');
    await openTab(page, 'General');
    await page.getByLabel('Name', { exact: true }).fill('Nope');
    await save(page);

    await expect(
      page.getByText("You don't have permission to change these fields.")
    ).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Save Changes' })
    ).toBeVisible();
  });

  test('keeps unsaved edits across a status action', async ({
    network,
    page
  }) => {
    let installed = true;
    network.use(
      gen.handleSetServerInstalled(async ({ request }) => {
        ({ installed } = await request.json());
        return ok();
      }),
      gen.handleServerInfo(({ params }) =>
        params.uuid === 'options'
          ? undefined
          : (HttpResponse.json({
              ...serverInfoOf(params.uuid),
              install_state: installed ? 'installed' : 'not_installed'
            }) as never)
      )
    );
    await gotoReady(page, '/servers/srv-survival/status');
    await openTab(page, 'General');
    await page.getByLabel('Name', { exact: true }).fill('Draft name');
    await openTab(page, 'Status');
    await action(page, 'Mark as not installed').click();
    // The refreshed server info is shown...
    await expect(action(page, 'Mark as installed')).toBeVisible();

    // ...without wiping the draft.
    await openTab(page, 'General');
    await expect(page.getByLabel('Name', { exact: true })).toHaveValue(
      'Draft name'
    );
    await expect(page.getByText('Unsaved changes')).toBeVisible();
  });
});

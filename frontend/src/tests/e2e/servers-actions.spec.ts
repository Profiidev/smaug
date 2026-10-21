import { expect } from '@playwright/test';
import { HttpResponse } from 'msw';
import * as gen from '$lib/client/msw.gen';
import { test } from '$test_helpers/e2e-fixture';
import { setupSession } from '$test_helpers/session';
import { expectNoHorizontalOverflow, gotoReady } from '$test_helpers/layout';
import { action, ok } from '$test_helpers/servers-e2e';

/** Expected enabled state per action for each mock server's status. */
const ALL = ['Reinstall', 'Mark as installed', 'Suspend', 'Transfer…'];
const matrix: {
  alert?: string;
  name: string;
  off: string[];
  on: string[];
  uuid: string;
}[] = [
  {
    name: 'Survival SMP',
    off: [],
    on: [
      'Reinstall',
      'Mark as not installed',
      'Suspend',
      'Transfer…',
      'Delete server'
    ],
    uuid: 'srv-survival'
  },
  {
    name: 'Creative',
    off: ['Reinstall', 'Transfer…', 'Delete server'],
    on: ['Mark as installed', 'Suspend'],
    uuid: 'srv-creative'
  },
  {
    alert: 'Install failed',
    name: 'Viking World',
    off: [],
    on: [...ALL, 'Delete server'],
    uuid: 'srv-viking'
  },
  {
    name: 'Fresh Server',
    off: [],
    on: [...ALL, 'Delete server'],
    uuid: 'srv-fresh'
  },
  {
    alert: 'Suspended',
    name: 'Banned Realm',
    off: ['Reinstall', 'Transfer…'],
    on: ['Mark as not installed', 'Unsuspend', 'Delete server'],
    uuid: 'srv-banned'
  }
];

test.describe('server status actions', () => {
  test.beforeEach(async ({ context }) => setupSession(context));

  for (const { alert, name, off, on, uuid } of matrix) {
    test(`${name} enables the actions its status allows`, async ({ page }) => {
      await gotoReady(page, `/servers/${uuid}/status`);
      await expect(
        page.getByRole('heading', { name: /Server:/ })
      ).toContainText(name);
      for (const label of on) {
        // oxlint-disable-next-line no-await-in-loop
        await expect(action(page, label)).toBeEnabled();
      }
      for (const label of off) {
        // oxlint-disable-next-line no-await-in-loop
        await expect(action(page, label)).toBeDisabled();
      }
      const canDelete = on.includes('Delete server');
      await expect(action(page, 'Delete')).toBeEnabled({ enabled: canDelete });
      if (alert) {
        await expect(
          page.locator('[data-slot=alert-title]', { hasText: alert })
        ).toBeVisible();
      }
      await expectNoHorizontalOverflow(page);
    });
  }

  test('reinstalls after confirming', async ({ network, page }) => {
    let calls = 0;
    network.use(
      gen.handleReinstallServer(() => {
        calls += 1;
        return ok();
      })
    );
    await gotoReady(page, '/servers/srv-survival/status');

    await action(page, 'Reinstall').click();
    const dialog = page.getByRole('dialog');
    await expect(dialog.getByText('Reinstall Survival SMP?')).toBeVisible();
    await dialog.getByRole('button', { name: 'Reinstall' }).click();

    await expect(page.getByText('Reinstalling Survival SMP')).toBeVisible();
    await expect(dialog).toBeHidden();
    expect(calls).toBe(1);
  });

  test('shows a reinstall failure', async ({ network, page }) => {
    network.use(
      gen.handleReinstallServer(
        () => new HttpResponse(null, { status: 409 }) as never
      )
    );
    await gotoReady(page, '/servers/srv-survival/status');

    await action(page, 'Reinstall').click();
    await page
      .getByRole('dialog')
      .getByRole('button', { name: 'Reinstall' })
      .click();
    await expect(page.getByText("Couldn't reinstall server.")).toBeVisible();
  });

  test('toggles the install status without confirming', async ({
    network,
    page
  }) => {
    const bodies: unknown[] = [];
    network.use(
      gen.handleSetServerInstalled(async ({ request }) => {
        bodies.push(await request.json());
        return ok();
      })
    );
    await gotoReady(page, '/servers/srv-fresh/status');
    await action(page, 'Mark as installed').click();
    await expect(
      page.getByText('Fresh Server marked as installed')
    ).toBeVisible();

    await gotoReady(page, '/servers/srv-survival/status');
    await action(page, 'Mark as not installed').click();
    await expect(
      page.getByText('Survival SMP marked as not installed')
    ).toBeVisible();
    expect(bodies).toEqual([{ installed: true }, { installed: false }]);
  });

  test('shows an install status failure', async ({ network, page }) => {
    network.use(
      gen.handleSetServerInstalled(
        () => new HttpResponse(null, { status: 500 }) as never
      )
    );
    await gotoReady(page, '/servers/srv-fresh/status');
    await action(page, 'Mark as installed').click();
    await expect(
      page.getByText("Couldn't change the install status.")
    ).toBeVisible();
  });

  test('suspends after confirming and unsuspends directly', async ({
    network,
    page
  }) => {
    const bodies: unknown[] = [];
    network.use(
      gen.handleSetServerSuspended(async ({ request }) => {
        bodies.push(await request.json());
        return ok();
      })
    );
    await gotoReady(page, '/servers/srv-survival/status');
    await action(page, 'Suspend').click();
    const dialog = page.getByRole('dialog');
    await expect(dialog.getByText('Suspend Survival SMP?')).toBeVisible();
    await dialog.getByRole('button', { name: 'Suspend' }).click();
    await expect(page.getByText('Survival SMP suspended')).toBeVisible();

    await gotoReady(page, '/servers/srv-banned/status');
    await action(page, 'Unsuspend').click();
    await expect(page.getByText('Banned Realm unsuspended')).toBeVisible();
    expect(bodies).toEqual([{ suspended: true }, { suspended: false }]);
  });

  test('shows suspend and unsuspend failures', async ({ network, page }) => {
    network.use(
      gen.handleSetServerSuspended(
        () => new HttpResponse(null, { status: 500 }) as never
      )
    );
    await gotoReady(page, '/servers/srv-survival/status');
    await action(page, 'Suspend').click();
    await page
      .getByRole('dialog')
      .getByRole('button', { name: 'Suspend' })
      .click();
    await expect(page.getByText("Couldn't suspend server.")).toBeVisible();

    await gotoReady(page, '/servers/srv-banned/status');
    await action(page, 'Unsuspend').click();
    await expect(page.getByText("Couldn't unsuspend server.")).toBeVisible();
  });
});

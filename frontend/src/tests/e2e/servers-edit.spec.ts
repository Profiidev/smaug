import { expect } from '@playwright/test';
import { HttpResponse, delay } from 'msw';
import * as gen from '$lib/client/msw.gen';
import { test } from '$test_helpers/e2e-fixture';
import { setupSession } from '$test_helpers/session';
import { expectNoHorizontalOverflow, gotoReady } from '$test_helpers/layout';
import {
  choose,
  combo,
  fieldErrors,
  ok,
  openTab,
  pick,
  recordUpdate,
  save,
  survivalBody,
  tab
} from '$test_helpers/servers-e2e';

test.describe('server edit (admin)', () => {
  test.beforeEach(async ({ context }) => setupSession(context));

  test('renders the status tab and fits the viewport', async ({ page }) => {
    await gotoReady(page, '/servers/srv-survival');

    await expect(page.getByRole('heading', { name: /Server:/ })).toContainText(
      'Survival SMP'
    );
    await expect(tab(page, 'Status')).toHaveAttribute('aria-selected', 'true');
    for (const name of [
      'General',
      'Startup',
      'Variables',
      'Resources',
      'Permissions'
    ]) {
      // oxlint-disable-next-line no-await-in-loop
      await expect(tab(page, name)).toBeVisible();
    }
    await expect(
      page.getByText('The server is installed and stopped.')
    ).toBeVisible();
    await expect(
      page.getByText('Usage is shown while the server is running.')
    ).toBeVisible();
    await expect(
      page.getByText('10.0.0.1:25566, 10.0.0.1:25567')
    ).toBeVisible();
    await expect(page.getByText('srv-survival', { exact: true })).toBeVisible();
    // The SDK transformer turns the created_at string into a Date.
    await expect(page.getByText(/^Created \d/)).toBeVisible();
    await expect(
      page.getByText('You have view-only access', { exact: true })
    ).toBeHidden();
    await expect(
      page.getByText('Some settings need an administrator')
    ).toBeHidden();
    // The save bar is hidden on the Status tab.
    await expect(
      page.getByRole('button', { name: 'Save Changes' })
    ).toBeHidden();
    await expectNoHorizontalOverflow(page);

    for (const name of [
      'General',
      'Startup',
      'Variables',
      'Resources',
      'Permissions'
    ]) {
      // oxlint-disable-next-line no-await-in-loop
      await openTab(page, name);
      // oxlint-disable-next-line no-await-in-loop
      await expect(
        page.getByRole('button', { name: 'Save Changes' })
      ).toBeVisible();
      // oxlint-disable-next-line no-await-in-loop
      await expectNoHorizontalOverflow(page);
    }
  });

  test('renders tabs and a disabled, empty form while loading', async ({
    network,
    page
  }) => {
    network.use(
      gen.handleServerInfo(async () => {
        await delay('infinite');
        return ok();
      })
    );
    await page.goto('/servers/srv-survival');

    await expect(page.locator('[aria-busy=true]')).toHaveCount(0);
    await expect(page.getByRole('tab')).toHaveText([
      'Status',
      'General',
      'Startup',
      'Variables',
      'Resources'
    ]);
    await expect(page.getByRole('alert')).toHaveCount(0);
    // Data never arrives, so networkidle can't signal hydration: retry the click.
    const general = page.getByRole('tab', { name: 'General' });
    await expect(async () => {
      await general.click();
      await expect(general).toHaveAttribute('aria-selected', 'true', {
        timeout: 500
      });
    }).toPass();
    await expect(page.getByLabel('Name', { exact: true })).toHaveValue('');
    await expect(page.getByLabel('Name', { exact: true })).toBeDisabled();
    await expect(
      page.getByRole('button', { name: 'Save Changes' })
    ).toBeDisabled();
  });

  test('redirects to the list when the server is missing', async ({ page }) => {
    await gotoReady(page, '/servers/srv-missing');

    await expect(page).toHaveURL(/\/servers$/);
    await expect(page.getByText('Server not found')).toBeVisible();
  });

  test('redirects to the list when loading fails', async ({
    network,
    page
  }) => {
    network.use(
      gen.handleServerInfo(
        () => new HttpResponse(null, { status: 500 }) as never
      )
    );
    await gotoReady(page, '/servers/srv-survival');

    await expect(page).toHaveURL(/\/servers$/);
    await expect(page.getByText('Failed to load server')).toBeVisible();
  });

  test('saves the general tab', async ({ network, page }) => {
    const update = recordUpdate();
    network.use(update.handler);
    await gotoReady(page, '/servers/srv-survival');
    await openTab(page, 'General');

    await expect(combo(page, 'Owner')).toHaveText('Ada Admin');
    await expect(combo(page, 'Node')).toBeDisabled();
    await expect(
      page.getByText('Use Transfer on the Status tab.')
    ).toBeVisible();
    await expect(page.getByLabel('IP address')).toHaveCount(3);

    await page.getByLabel('Name', { exact: true }).fill('Survival 2');
    await pick(page, 'Owner', 'Bob User');
    await page
      .getByRole('button', { name: 'Remove allocation' })
      .last()
      .click();
    await expect(page.getByText('Unsaved changes')).toBeVisible();
    await save(page);

    await expect(
      page.getByText('Server Survival 2 updated successfully')
    ).toBeVisible();
    await expect(page.getByRole('heading', { name: /Server:/ })).toContainText(
      'Survival 2'
    );
    await expect(page.getByText('Unsaved changes')).toBeHidden();
    expect(update.bodies).toEqual([
      {
        ...survivalBody,
        allocations: survivalBody.allocations.slice(0, 2),
        name: 'Survival 2',
        owner: 'user-1'
      }
    ]);
  });

  test('blocks invalid values and marks their tab', async ({
    network,
    page
  }) => {
    const update = recordUpdate();
    network.use(update.handler);
    await gotoReady(page, '/servers/srv-survival');
    await openTab(page, 'General');

    await page.getByLabel('Name', { exact: true }).fill('');
    await page.getByLabel('Port').nth(1).fill('25565');
    await openTab(page, 'Startup');
    await save(page);

    await expect(
      tab(page, 'General').getByRole('img', { name: 'Has errors' })
    ).toBeVisible();
    await openTab(page, 'General');
    await expect(page.getByText('Name is required.')).toBeVisible();
    await expect(page.getByText('Duplicate allocation.')).toBeVisible();
    expect(update.bodies).toEqual([]);
  });

  test('maps 400 field errors and jumps to the first tab', async ({
    network,
    page
  }) => {
    network.use(
      gen.handleUpdateServer(() =>
        fieldErrors(400, [
          { field: 'variables.MAX_PLAYERS', message: 'Too many players.' },
          { field: 'name', message: 'Name is taken.' }
        ])
      )
    );
    await gotoReady(page, '/servers/srv-survival');
    await openTab(page, 'Resources');
    await save(page);

    await expect(page.getByText("Couldn't save server")).toBeVisible();
    await expect(page.getByText('Fix the highlighted fields.')).toBeVisible();
    await expect(tab(page, 'General')).toHaveAttribute('aria-selected', 'true');
    await expect(page.getByText('Name is taken.')).toBeVisible();
    await expect(
      tab(page, 'Variables').getByRole('img', { name: 'Has errors' })
    ).toBeVisible();
    await openTab(page, 'Variables');
    await expect(page.getByText('Too many players.')).toBeVisible();
  });

  test('shows a limit error while its unlimited switch is on', async ({
    network,
    page
  }) => {
    network.use(
      gen.handleUpdateServer(() =>
        fieldErrors(400, [{ field: 'limits.disk', message: 'Disk too big.' }])
      )
    );
    await gotoReady(page, '/servers/srv-survival');
    await openTab(page, 'Resources');
    await expect(
      page.getByRole('switch', { name: 'Unlimited Storage' })
    ).toBeChecked();
    await save(page);

    await expect(page.getByText('Disk too big.')).toBeVisible();
    await expect(page.getByLabel('Storage limit (MiB)')).toBeHidden();
  });

  test('maps a 409 allocation conflict', async ({ network, page }) => {
    network.use(
      gen.handleUpdateServer(() =>
        fieldErrors(409, [
          { field: 'allocations.2', message: 'Allocation is in use.' }
        ])
      )
    );
    await gotoReady(page, '/servers/srv-survival');
    await openTab(page, 'Startup');
    await save(page);

    await expect(page.getByText("Couldn't save server")).toBeVisible();
    await expect(
      page.getByText('An allocation is already in use.')
    ).toBeVisible();
    await expect(tab(page, 'General')).toHaveAttribute('aria-selected', 'true');
    await expect(page.getByText('Allocation is in use.')).toBeVisible();
  });

  test('toasts unmapped errors in the description', async ({
    network,
    page
  }) => {
    network.use(
      gen.handleUpdateServer(() =>
        fieldErrors(400, [{ field: 'mystery', message: 'Odd failure.' }])
      )
    );
    await gotoReady(page, '/servers/srv-survival');
    await openTab(page, 'Startup');
    await save(page);

    await expect(page.getByText("Couldn't save server")).toBeVisible();
    await expect(page.getByText('Odd failure.')).toBeVisible();
    await expect(tab(page, 'Startup')).toHaveAttribute('aria-selected', 'true');
  });

  test('retries after a failed save', async ({ network, page }) => {
    let fail = true;
    const update = recordUpdate(() =>
      fail ? (new HttpResponse(null, { status: 500 }) as never) : ok()
    );
    network.use(update.handler);
    await gotoReady(page, '/servers/srv-survival');
    await openTab(page, 'General');
    await page.getByLabel('Name', { exact: true }).fill('Survival 3');
    await save(page);

    await expect(
      page.getByText("Couldn't save changes. Check your connection and retry.")
    ).toBeVisible();
    fail = false;
    await page.getByRole('button', { name: 'Retry' }).click();
    await expect(
      page.getByText('Server Survival 3 updated successfully')
    ).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Save Changes' })
    ).toBeVisible();
    expect(update.bodies).toHaveLength(2);
  });

  test('saves startup, variables and resources', async ({ network, page }) => {
    const update = recordUpdate();
    network.use(update.handler);
    await gotoReady(page, '/servers/srv-survival');

    await openTab(page, 'Startup');
    // Edit mode has no install switches.
    await expect(
      page.getByRole('switch', { name: 'Run install script' })
    ).toHaveCount(0);
    await choose(page, page.getByLabel('Startup command'), 'Aikar flags');
    await expect(page.getByText('Overridden')).toBeVisible();
    await choose(page, page.getByLabel('Image name'), 'Java 17');

    await openTab(page, 'Variables');
    await page.getByLabel('Max Players').fill('64');

    await openTab(page, 'Resources');
    await expect(page.getByLabel('CPU limit (millicores)')).toHaveValue('2000');
    await page.getByRole('switch', { name: 'Unlimited CPU' }).click();
    await page.getByRole('switch', { name: 'Unlimited Storage' }).click();
    await page.getByLabel('Storage limit (MiB)').fill('5000');
    await page.getByLabel('Backup limit').fill('0');
    await save(page);

    await expect(
      page.getByText('Server Survival SMP updated successfully')
    ).toBeVisible();
    expect(update.bodies).toEqual([
      {
        ...survivalBody,
        backup_limit: 0,
        environment: { ...survivalBody.environment, MAX_PLAYERS: '64' },
        image: 'ghcr.io/pelican-eggs/yolks:java_17',
        limits: { cpu: 0, disk: 5000, memory: 4096 },
        startup: 'java -XX:+UseG1GC -jar {{SERVER_JARFILE}}',
        startup_overridden: true
      }
    ]);
  });

  test('validates variables and limits on save', async ({ network, page }) => {
    const update = recordUpdate();
    network.use(update.handler);
    await gotoReady(page, '/servers/srv-survival');

    await openTab(page, 'Variables');
    await page.getByLabel('Max Players').fill('0');
    await openTab(page, 'Resources');
    await page.getByLabel('Memory limit (MiB)').fill('-5');
    await save(page);

    await expect(page.getByText('Memory must be 0 or more MiB.')).toBeVisible();
    await openTab(page, 'Variables');
    await expect(
      page.getByText('The Max Players field must be between 1 and 100.')
    ).toBeVisible();
    expect(update.bodies).toEqual([]);
  });

  test('changing the egg reloads startup and variables', async ({
    network,
    page
  }) => {
    const update = recordUpdate();
    network.use(update.handler);
    await gotoReady(page, '/servers/srv-survival');

    await openTab(page, 'Variables');
    await page.getByLabel('Max Players').fill('30');
    await openTab(page, 'General');
    await expect(
      page.getByText('Changing the egg reloads variables')
    ).toBeVisible();

    await pick(page, 'Egg', 'Valheim');
    const dialog = page.getByRole('alertdialog');
    await expect(dialog.getByText('Change egg?')).toBeVisible();
    await dialog.getByRole('button', { name: 'Keep egg' }).click();
    await expect(combo(page, 'Egg')).toHaveText('Paper');

    await pick(page, 'Egg', 'Valheim');
    await dialog.getByRole('button', { name: 'Change egg' }).click();
    await expect(combo(page, 'Egg')).toHaveText('Valheim');
    await openTab(page, 'Startup');
    await expect(page.getByRole('textbox', { name: 'Command' })).toHaveValue(
      './valheim_server'
    );
    await openTab(page, 'Variables');
    await expect(page.getByLabel('World Name')).toHaveValue('My World');
    await save(page);

    await expect(
      page.getByText('Server Survival SMP updated successfully')
    ).toBeVisible();
    expect(update.bodies).toEqual([
      {
        ...survivalBody,
        egg: 'egg-valheim',
        environment: { WORLD_NAME: 'My World' },
        image: 'ghcr.io/pelican-eggs/yolks:wine',
        startup: './valheim_server'
      }
    ]);
  });

  test('edits access grants', async ({ network, page }) => {
    const update = recordUpdate();
    network.use(update.handler);
    await gotoReady(page, '/servers/srv-survival');
    await openTab(page, 'Permissions');

    const targets = page
      .getByText('User or group', { exact: true })
      .locator('xpath=following-sibling::*[@role="combobox"]');
    await expect(targets).toHaveCount(2);
    await expect(targets.nth(0)).toHaveText('Bob User');
    await expect(targets.nth(1)).toHaveText('Staff');

    // A new grant needs a target and may not repeat one.
    await page.getByRole('button', { name: 'Add grant' }).click();
    await save(page);
    await expect(page.getByText('Select a user or group.')).toBeVisible();
    await targets.nth(2).click();
    await page.getByRole('option', { name: 'Staff' }).click();
    await save(page);
    await expect(page.getByText('Already granted.')).toBeVisible();

    await targets.nth(2).click();
    await page.getByRole('option', { name: 'Admins' }).click();
    await page
      .getByText('Access', { exact: true })
      .locator('xpath=following-sibling::*[@role="combobox"]')
      .nth(2)
      .click();
    await page.getByRole('option', { name: 'Write' }).click();
    await page.getByRole('button', { name: 'Remove grant' }).first().click();
    await save(page);

    await expect(
      page.getByText('Server Survival SMP updated successfully')
    ).toBeVisible();
    expect(update.bodies).toEqual([
      {
        ...survivalBody,
        grants: {
          groups: [
            { access: 'read', uuid: 'group-staff' },
            { access: 'write', uuid: 'group-admins' }
          ],
          users: []
        }
      }
    ]);

    // Removing every grant shows the empty state.
    await page.getByRole('button', { name: 'Remove grant' }).first().click();
    await page.getByRole('button', { name: 'Remove grant' }).first().click();
    await expect(page.getByText('No grants')).toBeVisible();
  });

  test('maps grant field errors to the permissions tab', async ({
    network,
    page
  }) => {
    network.use(
      gen.handleUpdateServer(() =>
        fieldErrors(400, [
          { field: 'grants.groups.0', message: 'Group is gone.' }
        ])
      )
    );
    await gotoReady(page, '/servers/srv-survival');
    await openTab(page, 'General');
    await save(page);

    await expect(tab(page, 'Permissions')).toHaveAttribute(
      'aria-selected',
      'true'
    );
    await expect(page.getByText('Group is gone.')).toBeVisible();
  });
});

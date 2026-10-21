import { expect } from '@playwright/test';
import { HttpResponse } from 'msw';
import * as gen from '$lib/client/msw.gen';
import { test } from '$test_helpers/e2e-fixture';
import { setupSession } from '$test_helpers/session';
import { gotoReady } from '$test_helpers/layout';
import {
  fillGeneral,
  next,
  pick,
  previous,
  stageTitle,
  submitWith
} from '$test_helpers/servers-e2e';
import { serverOptions } from '../mocks/e2e/data';

test.describe('server create validation', () => {
  test.beforeEach(async ({ context }) => setupSession(context));

  test('blocks an empty General stage', async ({ network, page }) => {
    let called = false;
    network.use(
      gen.handleCreateServer(() => {
        called = true;
        return HttpResponse.json({ uuid: 'srv-new' }) as never;
      })
    );
    await gotoReady(page, '/servers/create');

    await next(page);

    await expect(page.getByText('Name is required.')).toBeVisible();
    await expect(page.getByText('Node is required.')).toBeVisible();
    await expect(page.getByText('Egg is required.')).toBeVisible();
    await expect(
      page.getByText('Enter a valid IPv4 or IPv6 address.')
    ).toBeVisible();
    await expect(stageTitle(page, 'General')).toBeVisible();
    expect(called).toBe(false);
  });

  test('requires an owner when the user is not selectable', async ({
    network,
    page
  }) => {
    network.use(
      gen.handleServerOptions(
        () =>
          HttpResponse.json({
            ...serverOptions.default,
            users: [{ name: 'Bob User', uuid: 'user-1' }]
          }) as never
      )
    );
    await gotoReady(page, '/servers/create');

    await fillGeneral(page);
    await next(page);
    await expect(page.getByText('Owner is required.')).toBeVisible();

    await pick(page, 'Owner', 'Bob User');
    await next(page);
    await expect(stageTitle(page, 'Startup')).toBeVisible();
  });

  test('validates allocations', async ({ page }) => {
    await gotoReady(page, '/servers/create');
    await fillGeneral(page);

    // A single allocation cannot be removed.
    await expect(
      page.getByRole('button', { name: 'Remove allocation' })
    ).toBeDisabled();

    // Duplicates are flagged (IP compare ignores case and whitespace).
    await page.getByRole('button', { name: 'Add allocation' }).click();
    await page.getByLabel('Port').nth(1).fill('25565');
    await next(page);
    await expect(page.getByText('Duplicate allocation.')).toBeVisible();

    // 0.0.0.0 already covers every IPv4 address on that port.
    await submitWith(
      page,
      async () => page.getByLabel('IP address').nth(1).fill('127.0.0.1'),
      page.getByText('Overlaps another allocation on the same port.')
    );

    // Port range.
    await submitWith(
      page,
      async () => page.getByLabel('Port').nth(1).fill('80'),
      page.getByText('Port must be between 1024 and 65535.')
    );
    await submitWith(
      page,
      async () => page.getByLabel('Port').nth(1).fill('70000'),
      page.getByText('Port must be between 1024 and 65535.')
    );

    // Invalid IP.
    await submitWith(
      page,
      async () => {
        await page.getByLabel('Port').nth(1).fill('25566');
        await page.getByLabel('IP address').nth(1).fill('not-an-ip');
      },
      page.getByText('Enter a valid IPv4 or IPv6 address.')
    );

    // Removing the bad row unblocks the stage; IPv6 is accepted.
    await page
      .getByRole('button', { name: 'Remove allocation' })
      .nth(1)
      .click();
    await page.getByLabel('IP address').first().fill('::1');
    await next(page);
    await expect(stageTitle(page, 'Startup')).toBeVisible();
  });

  test('validates name length', async ({ page }) => {
    await gotoReady(page, '/servers/create');
    await fillGeneral(page);
    await page.getByLabel('Name', { exact: true }).fill('x'.repeat(192));
    await next(page);

    await expect(
      page.getByText('Name must be 191 characters or fewer.')
    ).toBeVisible();
  });

  test('validates startup command and image', async ({ page }) => {
    await gotoReady(page, '/servers/create');
    await fillGeneral(page);
    await next(page);

    await page.getByRole('textbox', { name: 'Command' }).fill('');
    await page.getByLabel('Image', { exact: true }).fill('  ');
    await next(page);

    await expect(page.getByText('Startup command is required.')).toBeVisible();
    await expect(page.getByText('Image is required.')).toBeVisible();
    await expect(stageTitle(page, 'Startup')).toBeVisible();
  });

  test('validates variables against egg rules', async ({ page }) => {
    await gotoReady(page, '/servers/create');
    await fillGeneral(page);
    await next(page);
    await next(page);

    await expect(
      page.getByText('required|integer|between:1,100')
    ).toBeVisible();
    const players = page.getByLabel('Max Players');
    const jar = page.getByLabel('Server Jar File');
    const version = page.getByLabel('Version');

    await players.fill('');
    await jar.fill('a'.repeat(21));
    await version.fill('1.8');
    await next(page);
    await expect(
      page.getByText('The Max Players field is required.')
    ).toBeVisible();
    await expect(
      page.getByText(
        'The Server Jar File field must not be greater than 20 characters.'
      )
    ).toBeVisible();
    await expect(
      page.getByText('The selected Version is invalid.')
    ).toBeVisible();

    await submitWith(
      page,
      async () => players.fill('abc'),
      page.getByText('The Max Players field must be an integer.')
    );

    await submitWith(
      page,
      async () => players.fill('500'),
      page.getByText('The Max Players field must be between 1 and 100.')
    );

    // `nullable` lets an empty value through.
    await submitWith(
      page,
      async () => {
        await players.fill('10');
        await jar.fill('server.jar');
        await version.fill('');
      },
      stageTitle(page, 'Resources')
    );
  });

  test('validates a regex rule and shows eggs without variables', async ({
    page
  }) => {
    await gotoReady(page, '/servers/create');
    await fillGeneral(page, 'Valheim');
    await next(page);
    await next(page);

    await page.getByLabel('World Name').fill('World 42');
    await next(page);
    await expect(
      page.getByText('The World Name field format is invalid.')
    ).toBeVisible();

    await previous(page);
    await previous(page);
    // Edited variables: picking another egg asks first.
    await pick(page, 'Egg', 'Bare');
    await page.getByRole('button', { name: 'Change egg' }).click();
    await next(page);
    await expect(page.getByRole('textbox', { name: 'Command' })).toHaveValue(
      './run'
    );
    await next(page);
    await expect(page.getByText('No variables', { exact: true })).toBeVisible();
  });

  test('Previous never validates and keeps entered values', async ({
    page
  }) => {
    await gotoReady(page, '/servers/create');
    await fillGeneral(page);
    await next(page);

    await page.getByRole('textbox', { name: 'Command' }).fill('');
    await previous(page);
    await expect(stageTitle(page, 'General')).toBeVisible();
    await expect(page.getByText('Startup command is required.')).toBeHidden();
    await expect(page.getByLabel('Name', { exact: true })).toHaveValue(
      'Survival'
    );

    await next(page);
    await expect(page.getByRole('textbox', { name: 'Command' })).toHaveValue(
      ''
    );
    await next(page);
    await expect(page.getByText('Startup command is required.')).toBeVisible();
  });

  test('validates resource limits', async ({ page }) => {
    await gotoReady(page, '/servers/create');
    await fillGeneral(page);
    await next(page);
    await next(page);
    await next(page);

    // Unlimited by default: no inputs.
    await expect(page.getByText('No cpu limit.')).toBeVisible();
    await expect(page.getByLabel('CPU limit (millicores)')).toBeHidden();

    await page.getByRole('switch', { name: 'Unlimited CPU' }).click();
    await page.getByRole('switch', { name: 'Unlimited Memory' }).click();
    await page.getByRole('switch', { name: 'Unlimited Storage' }).click();
    await expect(page.getByLabel('CPU limit (millicores)')).toHaveValue('1000');
    await expect(page.getByLabel('Memory limit (MiB)')).toHaveValue('4096');
    await expect(page.getByLabel('Storage limit (MiB)')).toHaveValue('10240');
    await expect(page.getByText('1000 millicores is one core.')).toBeVisible();

    await page.getByLabel('CPU limit (millicores)').fill('-1');
    await page.getByLabel('Memory limit (MiB)').fill('');
    await page.getByLabel('Backup limit').fill('-2');
    await page.getByRole('button', { exact: true, name: 'Create' }).click();
    await expect(
      page.getByText('CPU must be 0 or more millicores.')
    ).toBeVisible();
    await expect(page.getByText('Memory is required.')).toBeVisible();
    await expect(
      page.getByText('Backup limit must be 0 or more backups.')
    ).toBeVisible();
    await expect(page).toHaveURL(/\/servers\/create$/);

    // Hiding an invalid limit skips its check.
    await page.getByRole('switch', { name: 'Unlimited CPU' }).click();
    await page.getByRole('switch', { name: 'Unlimited Memory' }).click();
    await page.getByLabel('Backup limit').fill('0');
    await page.getByRole('button', { exact: true, name: 'Create' }).click();
    await expect(page).toHaveURL(/\/servers\/srv-new\/status$/);
  });
});

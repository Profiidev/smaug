import { expect } from '@playwright/test';
import { HttpResponse } from 'msw';
import * as gen from '$lib/client/msw.gen';
import { test } from '$test_helpers/e2e-fixture';
import { setupSession } from '$test_helpers/session';
import { expectNoHorizontalOverflow, gotoReady } from '$test_helpers/layout';
import {
  choose,
  combo,
  fieldErrors,
  fillGeneral,
  next,
  pick,
  previous,
  stageTitle
} from '$test_helpers/servers-e2e';

test.describe('server create', () => {
  test.beforeEach(async ({ context }) => setupSession(context));

  test('keeps the header right above the wizard card', async ({ page }) => {
    await gotoReady(page, '/servers/create');
    await expect(page.getByLabel('Name', { exact: true })).toBeVisible();
    const header = (await page
      .getByRole('link', { name: 'Back to servers' })
      .boundingBox())!;
    const card = (await page
      .locator('[data-slot=card]')
      .first()
      .boundingBox())!;
    // Directly above the card (gap-4), not pinned to the page top.
    const gap = card.y - (header.y + header.height);
    expect(gap).toBeGreaterThanOrEqual(0);
    expect(gap).toBeLessThanOrEqual(24);
    // Left-aligned with the card; mobile shifts it past the sidebar trigger (ml-7).
    expect(header.x - card.x).toBeGreaterThanOrEqual(0);
    expect(header.x - card.x).toBeLessThanOrEqual(29);
  });

  test('creates a server and posts the form', async ({ network, page }) => {
    const bodies: unknown[] = [];
    network.use(
      gen.handleCreateServer(async ({ request }) => {
        bodies.push(await request.json());
        return HttpResponse.json({ uuid: 'srv-new' }) as never;
      })
    );
    await gotoReady(page, '/servers/create');

    // General: the owner defaults to the current user.
    await expect(stageTitle(page, 'General')).toBeVisible();
    await expect(combo(page, 'Owner')).toHaveText('Ada Admin');
    await fillGeneral(page);
    await page.getByRole('button', { name: 'Add allocation' }).click();
    // A new row copies the last IP and takes the next port.
    await expect(page.getByLabel('IP address').nth(1)).toHaveValue('0.0.0.0');
    await expect(page.getByLabel('Port').nth(1)).toHaveValue('25566');
    await expectNoHorizontalOverflow(page);
    await next(page);

    // Startup: egg defaults loaded; skip the install script.
    await expect(stageTitle(page, 'Startup')).toBeVisible();
    await expect(page.getByRole('textbox', { name: 'Command' })).toHaveValue(
      'java -jar {{SERVER_JARFILE}}'
    );
    await expect(page.getByLabel('Image', { exact: true })).toHaveValue(
      'ghcr.io/pelican-eggs/yolks:java_21'
    );
    await page.getByRole('switch', { name: 'Run install script' }).click();
    await expectNoHorizontalOverflow(page);
    await next(page);

    // Variables: egg defaults, sorted.
    await expect(stageTitle(page, 'Variables')).toBeVisible();
    await expect(page.getByLabel('Server Jar File')).toHaveValue('server.jar');
    await page.getByLabel('Max Players').fill('50');
    await expectNoHorizontalOverflow(page);
    await next(page);

    // Resources: limit CPU, keep the rest unlimited.
    await expect(stageTitle(page, 'Resources')).toBeVisible();
    await page.getByRole('switch', { name: 'Unlimited CPU' }).click();
    // The limit input sits inside its switch's bordered row.
    await expect(
      page
        .locator('[data-slot=item]', {
          has: page.getByRole('switch', { name: 'Unlimited CPU' })
        })
        .getByLabel('CPU limit (millicores)')
    ).toBeVisible();
    await page.getByLabel('CPU limit (millicores)').fill('1500');
    await page.getByLabel('Backup limit').fill('3');
    await expectNoHorizontalOverflow(page);
    await page.getByRole('button', { exact: true, name: 'Create' }).click();

    await expect(page).toHaveURL(/\/servers\/srv-new\/status$/);
    await expect(page.getByText('Server created successfully.')).toBeVisible();
    expect(bodies).toEqual([
      {
        allocations: [
          { ip: '0.0.0.0', port: 25_565 },
          { ip: '0.0.0.0', port: 25_566 }
        ],
        backup_limit: 3,
        egg: 'egg-paper',
        environment: {
          MAX_PLAYERS: '50',
          SERVER_JARFILE: 'server.jar',
          VERSION: 'latest'
        },
        image: 'ghcr.io/pelican-eggs/yolks:java_21',
        limits: { cpu: 1500, disk: 0, memory: 0 },
        name: 'Survival',
        node: 'node-1',
        owner: 'user-admin',
        restart_on_crash: true,
        skip_scripts: true,
        start_on_completion: true,
        startup: 'java -jar {{SERVER_JARFILE}}',
        startup_overridden: false,
        stop_timeout: 60
      }
    ]);
  });

  test('overrides the startup command and resets it', async ({ page }) => {
    await gotoReady(page, '/servers/create');
    await fillGeneral(page);
    await next(page);

    const command = page.getByRole('textbox', { name: 'Command' });
    await expect(page.getByText('Follows the egg.')).toBeVisible();

    // Picking a non-default egg command overrides.
    await choose(page, page.getByLabel('Startup command'), 'Aikar flags');
    await expect(command).toHaveValue(
      'java -XX:+UseG1GC -jar {{SERVER_JARFILE}}'
    );
    await expect(page.getByText('Overridden')).toBeVisible();

    await page.getByRole('button', { name: 'Reset to default' }).click();
    await expect(command).toHaveValue('java -jar {{SERVER_JARFILE}}');
    await expect(page.getByText('Overridden')).toBeHidden();
    await expect(page.getByLabel('Startup command')).toHaveText('Default');

    // Typing overrides and switches the select to Custom.
    await command.fill('java -jar custom.jar');
    await expect(page.getByText('Overridden')).toBeVisible();
    await expect(
      page.getByText('This server uses its own command.')
    ).toBeVisible();
    await expect(page.getByLabel('Startup command')).toHaveText('Custom');

    // Picking Custom keeps the text editable and marks it overridden.
    await page.getByRole('button', { name: 'Reset to default' }).click();
    await choose(page, page.getByLabel('Startup command'), 'Custom');
    await expect(page.getByLabel('Startup command')).toHaveText('Custom');
    await expect(command).toHaveValue('java -jar {{SERVER_JARFILE}}');
    await expect(page.getByText('Overridden')).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Reset to default' })
    ).toBeVisible();
    await expect(command).toBeEditable();
    await command.fill('java -jar picked.jar');
    await expect(page.getByLabel('Startup command')).toHaveText('Custom');
  });

  test('picks an egg image or a custom one', async ({ page }) => {
    await gotoReady(page, '/servers/create');
    await fillGeneral(page);
    await next(page);

    const image = page.getByLabel('Image', { exact: true });
    const select = page.getByLabel('Image name');
    await choose(page, select, 'Java 17');
    await expect(image).toHaveValue('ghcr.io/pelican-eggs/yolks:java_17');
    await expect(page.getByText('Custom image. Must be')).toBeHidden();

    await image.fill('registry.example/custom:1');
    await expect(select).toHaveText('Custom image');
    await expect(page.getByText('Custom image. Must be')).toBeVisible();

    // Picking Custom image switches to custom mode and keeps the field.
    await choose(page, select, 'Java 17');
    await choose(page, select, 'Custom image');
    await expect(select).toHaveText('Custom image');
    await expect(image).toHaveValue('ghcr.io/pelican-eggs/yolks:java_17');
    await expect(page.getByText('Custom image. Must be')).toBeVisible();
    await image.fill('');
    await next(page);
    await expect(page.getByText('Image is required.')).toBeVisible();
    await expect(select).toHaveText('Custom image');
  });

  test('confirms an egg change that discards edited variables', async ({
    page
  }) => {
    await gotoReady(page, '/servers/create');
    await fillGeneral(page);
    await next(page);
    await next(page);
    await page.getByLabel('Max Players').fill('42');
    await previous(page);
    await previous(page);

    // Keeping the egg leaves everything as it was.
    await pick(page, 'Egg', 'Valheim');
    const dialog = page.getByRole('alertdialog');
    await expect(dialog.getByText('Change egg?')).toBeVisible();
    await dialog.getByRole('button', { name: 'Keep egg' }).click();
    await expect(combo(page, 'Egg')).toHaveText('Paper');
    await next(page);
    await next(page);
    await expect(page.getByLabel('Max Players')).toHaveValue('42');

    // Changing reloads the startup and variables stages.
    await previous(page);
    await previous(page);
    await pick(page, 'Egg', 'Valheim');
    await dialog.getByRole('button', { name: 'Change egg' }).click();
    await expect(combo(page, 'Egg')).toHaveText('Valheim');
    await next(page);
    await expect(page.getByRole('textbox', { name: 'Command' })).toHaveValue(
      './valheim_server'
    );
    await expect(page.getByLabel('Image', { exact: true })).toHaveValue(
      'ghcr.io/pelican-eggs/yolks:wine'
    );
    await next(page);
    await expect(page.getByLabel('World Name')).toHaveValue('My World');
    await expect(page.getByLabel('Max Players')).toBeHidden();
  });

  test('changes the egg without asking when variables are untouched', async ({
    page
  }) => {
    await gotoReady(page, '/servers/create');
    await fillGeneral(page);
    await next(page);
    await next(page);
    await previous(page);
    await previous(page);

    await pick(page, 'Egg', 'Valheim');
    await expect(page.getByRole('alertdialog')).toBeHidden();
    await next(page);
    await expect(page.getByRole('textbox', { name: 'Command' })).toHaveValue(
      './valheim_server'
    );
  });

  test('maps 400 field errors to their stages', async ({ network, page }) => {
    network.use(
      gen.handleCreateServer(() =>
        fieldErrors(400, [
          { field: 'variables.MAX_PLAYERS', message: 'Too many players.' },
          { field: 'name', message: 'Name is taken.' },
          { field: 'backup_limit', message: 'Too many backups.' }
        ])
      )
    );
    await gotoReady(page, '/servers/create');
    await fillGeneral(page);
    await next(page);
    await next(page);
    await next(page);
    await page.getByRole('button', { exact: true, name: 'Create' }).click();

    // Jumps back to the first stage with an error.
    await expect(
      page.getByText("Couldn't create server. Fix the highlighted fields.")
    ).toBeVisible();
    await expect(stageTitle(page, 'General')).toBeVisible();
    await expect(page.getByText('Name is taken.')).toBeVisible();

    await next(page);
    await next(page);
    await expect(page.getByText('Too many players.')).toBeVisible();
    await next(page);
    await expect(page.getByText('Too many backups.')).toBeVisible();
    await expect(page).toHaveURL(/\/servers\/create$/);
  });

  test('maps a 409 allocation conflict', async ({ network, page }) => {
    network.use(
      gen.handleCreateServer(() =>
        fieldErrors(409, [
          { field: 'allocations.1', message: 'Allocation is in use.' }
        ])
      )
    );
    await gotoReady(page, '/servers/create');
    await fillGeneral(page);
    await page.getByRole('button', { name: 'Add allocation' }).click();
    await next(page);
    await next(page);
    await next(page);
    await page.getByRole('button', { exact: true, name: 'Create' }).click();

    await expect(
      page.getByText("Couldn't create server: an allocation is already in use.")
    ).toBeVisible();
    await expect(stageTitle(page, 'General')).toBeVisible();
    await expect(page.getByText('Allocation is in use.')).toBeVisible();
  });

  test('shows a list-level allocation error', async ({ network, page }) => {
    network.use(
      gen.handleCreateServer(() =>
        fieldErrors(400, [
          { field: 'allocations', message: 'Too many allocations.' }
        ])
      )
    );
    await gotoReady(page, '/servers/create');
    await fillGeneral(page);
    await next(page);
    await next(page);
    await next(page);
    await page.getByRole('button', { exact: true, name: 'Create' }).click();

    await expect(stageTitle(page, 'General')).toBeVisible();
    await expect(page.getByText('Too many allocations.')).toBeVisible();
  });

  test('toasts errors no field shows', async ({ network, page }) => {
    let status = 400;
    network.use(
      gen.handleCreateServer(() =>
        status === 400
          ? fieldErrors(400, [{ field: 'mystery', message: 'Odd failure.' }])
          : (new HttpResponse(null, { status }) as never)
      )
    );
    await gotoReady(page, '/servers/create');
    await fillGeneral(page);
    await next(page);
    await next(page);
    await next(page);
    await page.getByRole('button', { exact: true, name: 'Create' }).click();

    await expect(page.getByText('Odd failure.')).toBeVisible();
    await expect(stageTitle(page, 'Resources')).toBeVisible();

    status = 500;
    await page.getByRole('button', { name: 'Retry' }).click();
    await expect(page.getByText('Error creating server.')).toBeVisible();
    await expect(page).toHaveURL(/\/servers\/create$/);
  });

  test('Cancel and Back return to the list', async ({ page }) => {
    await gotoReady(page, '/servers/create');
    await page.getByRole('link', { name: 'Cancel' }).click();
    await expect(page).toHaveURL(/\/servers$/);

    await gotoReady(page, '/servers/create');
    await page.getByRole('link', { name: 'Back to servers' }).click();
    await expect(page).toHaveURL(/\/servers$/);
  });

  test('warns when there are no nodes or eggs', async ({ context, page }) => {
    await setupSession(context, 'empty');
    await gotoReady(page, '/servers/create');

    await expect(
      page.getByText('Nothing to create a server on yet')
    ).toBeVisible();
    await expectNoHorizontalOverflow(page);
  });

  test('redirects to the list when options fail', async ({ network, page }) => {
    network.use(
      gen.handleServerOptions(
        () => new HttpResponse(null, { status: 500 }) as never
      )
    );
    await gotoReady(page, '/servers/create');

    await expect(page).toHaveURL(/\/servers$/);
    await expect(page.getByText('Failed to load server options')).toBeVisible();
  });
});

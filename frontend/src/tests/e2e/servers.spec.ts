import { type Locator, type Page, expect } from '@playwright/test';
import { HttpResponse, delay } from 'msw';
import * as gen from '$lib/client/msw.gen';
import { test } from '$test_helpers/e2e-fixture';
import { setupSession } from '$test_helpers/session';
import { expectNoHorizontalOverflow, gotoReady } from '$test_helpers/layout';
import { serverInfoOf, serverOptions } from '../mocks/e2e/data';

const ok = () => new HttpResponse(null, { status: 200 }) as never;

const fieldErrors = (
  status: number,
  errors: { field: string; message: string }[]
) => HttpResponse.json({ errors }, { status }) as never;

const row = (page: Page, name: string) =>
  page.getByRole('row', { name: new RegExp(name) });

const next = async (page: Page) =>
  page.getByRole('button', { name: 'Next' }).click();

const previous = async (page: Page) =>
  page.getByRole('button', { name: 'Previous' }).click();

/**
 * Runs `fill`, submits the stage and waits for `expected`. Retried: right
 * after an invalid submit superforms can still write the submitted values
 * back, swallowing fills made in that window (seen on Mobile Chrome).
 */
const submitWith = async (
  page: Page,
  fill: () => Promise<unknown>,
  expected: Locator
) =>
  expect(async () => {
    await fill();
    await next(page);
    await expect(expected).toBeVisible({ timeout: 1000 });
  }).toPass();

const stageTitle = (page: Page, title: string) =>
  page.locator('[data-slot=card-title]', { hasText: title });

/**
 * The FormSelect trigger labelled `label`. Its label is not linked to the
 * combobox (no accessible name), so locate it next to the label text.
 */
const combo = (scope: Page | Locator, label: string) =>
  scope
    .getByText(label, { exact: true })
    .locator('xpath=following-sibling::*[@role="combobox"]');

/** Picks `option` in the single FormSelect labelled `label`. */
const pick = async (page: Page, label: string, option: string) => {
  await combo(page, label).click();
  await page.getByRole('option', { name: option }).click();
};

/** Fills the General stage with valid values (owner is prefilled). */
const fillGeneral = async (page: Page, egg = 'Paper') => {
  await page.getByLabel('Name', { exact: true }).fill('Survival');
  await pick(page, 'Node', 'Node One');
  await pick(page, 'Egg', egg);
  await page.getByLabel('IP address').first().fill('0.0.0.0');
};

test.describe('servers list', () => {
  test.beforeEach(async ({ context }) => setupSession(context));

  test('lists servers with status and allocations', async ({ page }) => {
    await gotoReady(page, '/servers');

    await expect(page.getByRole('heading', { name: 'Servers' })).toBeVisible();
    await expect(page.getByText('5 servers')).toBeVisible();

    const survival = row(page, 'Survival SMP');
    await expect(survival.getByText('Offline')).toBeVisible();
    await expect(survival.getByText('Node One')).toBeVisible();
    await expect(survival.getByText('10.0.0.1:25565 +2')).toBeVisible();
    await expect(survival.getByText('Paper')).toBeVisible();
    await expect(
      survival.getByRole('link', { name: 'Survival SMP' })
    ).toHaveAttribute('href', '/servers/srv-survival');

    await expect(row(page, 'Creative').getByText('Installing')).toBeVisible();
    await expect(
      row(page, 'Viking World').getByText('Install failed')
    ).toBeVisible();
    // IPv6 allocations are bracketed.
    await expect(
      row(page, 'Viking World').getByText('[2001:db8::1]:2456')
    ).toBeVisible();
    await expect(
      row(page, 'Fresh Server').getByText('Not installed')
    ).toBeVisible();
    await expect(
      row(page, 'Banned Realm').getByText('Suspended')
    ).toBeVisible();

    await expect(
      page.getByRole('link', { name: 'Create server' })
    ).toHaveAttribute('href', '/servers/create');
    await expectNoHorizontalOverflow(page);
  });

  test('searches by name, node and allocation', async ({ page }) => {
    await gotoReady(page, '/servers');
    const search = page.getByPlaceholder('Search...');

    await search.fill('viking');
    await expect(row(page, 'Viking World')).toBeVisible();
    await expect(row(page, 'Survival SMP')).toBeHidden();

    await search.fill('node two');
    await expect(row(page, 'Creative')).toBeVisible();
    await expect(row(page, 'Fresh Server')).toBeVisible();
    await expect(row(page, 'Viking World')).toBeHidden();

    await search.fill('25566');
    await expect(row(page, 'Survival SMP')).toBeHidden();
    await search.fill('+2');
    await expect(row(page, 'Survival SMP')).toBeVisible();
    await expect(row(page, 'Creative')).toBeHidden();

    await search.fill('nothing-matches');
    await expect(page.getByText('No results.')).toBeVisible();
  });

  test('filters by status', async ({ page }) => {
    await gotoReady(page, '/servers');
    const filter = page.getByLabel('Filter by status');

    await filter.selectOption({ label: 'Suspended' });
    await expect(row(page, 'Banned Realm')).toBeVisible();
    await expect(row(page, 'Survival SMP')).toBeHidden();
    await expect(page.getByText('1 server', { exact: true })).toBeVisible();

    await filter.selectOption({ label: 'All statuses' });
    await expect(page.getByText('5 servers')).toBeVisible();
  });

  test('shows the empty state', async ({ context, page }) => {
    await setupSession(context, 'empty');
    await gotoReady(page, '/servers');

    await expect(page.getByText('No servers yet')).toBeVisible();
    // The empty state carries the only create action and no filter.
    await expect(page.getByRole('link', { name: 'Create server' })).toHaveCount(
      1
    );
    await expect(page.getByLabel('Filter by status')).toBeHidden();
    await expectNoHorizontalOverflow(page);
  });

  test('shows an empty table when the list fails', async ({
    network,
    page
  }) => {
    network.use(
      gen.handleListServers(
        () => new HttpResponse(null, { status: 500 }) as never
      )
    );
    await gotoReady(page, '/servers');

    await expect(page.getByText('No servers yet')).toBeVisible();
  });

  test('ignores an unknown ?error= but strips it', async ({ page }) => {
    await gotoReady(page, '/servers?error=bogus');

    await expect(page).toHaveURL(/\/servers$/);
    await expect(page.locator('[data-sonner-toast]')).toHaveCount(0);
  });

  test('deletes a server', async ({ network, page }) => {
    const deleted: unknown[] = [];
    network.use(
      gen.handleDeleteServer(({ params }) => {
        deleted.push(params.uuid);
        return ok();
      })
    );
    await gotoReady(page, '/servers');

    await row(page, 'Viking World').getByRole('button').last().click();
    const dialog = page.getByRole('dialog');
    await expect(dialog.getByText('Delete server Viking World?')).toBeVisible();
    await dialog.getByRole('button', { name: 'Delete' }).click();

    await expect(
      page.getByText('Server Viking World deleted successfully')
    ).toBeVisible();
    await expect(dialog).toBeHidden();
    expect(deleted).toEqual(['srv-viking']);
  });

  test('shows an error when delete fails', async ({ network, page }) => {
    network.use(
      gen.handleDeleteServer(
        () => new HttpResponse(null, { status: 500 }) as never
      )
    );
    await gotoReady(page, '/servers');

    await row(page, 'Viking World').getByRole('button').last().click();
    await page
      .getByRole('dialog')
      .getByRole('button', { name: 'Delete' })
      .click();

    await expect(page.getByText('Failed to delete server')).toBeVisible();
    await expect(page).toHaveURL(/\/servers$/);
  });

  test('disables delete while the status forbids it', async ({ page }) => {
    await gotoReady(page, '/servers');

    // Installing
    await expect(
      row(page, 'Creative').getByRole('button').last()
    ).toBeDisabled();
    await expect(
      row(page, 'Banned Realm').getByRole('button').last()
    ).toBeEnabled();
  });

  test('edit action links to the server', async ({ page }) => {
    await gotoReady(page, '/servers');

    const edit = row(page, 'Viking World').getByRole('link').last();
    await expect(edit).toHaveAttribute('href', '/servers/srv-viking');
    await edit.click();
    await expect(page).toHaveURL(/\/servers\/srv-viking$/);
  });
});

test.describe('server create', () => {
  test.beforeEach(async ({ context }) => setupSession(context));

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
    await page.getByLabel('CPU limit (millicores)').fill('1500');
    await page.getByLabel('Backup limit').fill('3');
    await expectNoHorizontalOverflow(page);
    await page.getByRole('button', { name: 'Create server' }).click();

    await expect(page).toHaveURL(/\/servers\/srv-new$/);
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
        skip_scripts: true,
        start_on_completion: true,
        startup: 'java -jar {{SERVER_JARFILE}}',
        startup_overridden: false
      }
    ]);
  });

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

  test('overrides the startup command and resets it', async ({ page }) => {
    await gotoReady(page, '/servers/create');
    await fillGeneral(page);
    await next(page);

    const command = page.getByRole('textbox', { name: 'Command' });
    await expect(page.getByText('Follows the egg.')).toBeVisible();

    // Picking a non-default egg command overrides.
    await page
      .getByLabel('Startup command')
      .selectOption({ label: 'Aikar flags' });
    await expect(command).toHaveValue(
      'java -XX:+UseG1GC -jar {{SERVER_JARFILE}}'
    );
    await expect(page.getByText('Overridden')).toBeVisible();

    await page.getByRole('button', { name: 'Reset to default' }).click();
    await expect(command).toHaveValue('java -jar {{SERVER_JARFILE}}');
    await expect(page.getByText('Overridden')).toBeHidden();
    await expect(page.getByLabel('Startup command')).toHaveValue('Default');

    // Typing overrides and switches the select to Custom.
    await command.fill('java -jar custom.jar');
    await expect(page.getByText('Overridden')).toBeVisible();
    await expect(
      page.getByText('This server uses its own command.')
    ).toBeVisible();
    await expect(page.getByLabel('Startup command')).toHaveValue('');

    // Picking Custom keeps the text editable and marks it overridden.
    await page.getByRole('button', { name: 'Reset to default' }).click();
    await page.getByLabel('Startup command').selectOption({ label: 'Custom' });
    await expect(page.getByLabel('Startup command')).toHaveValue('');
    await expect(command).toHaveValue('java -jar {{SERVER_JARFILE}}');
    await expect(page.getByText('Overridden')).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Reset to default' })
    ).toBeVisible();
    await expect(command).toBeEditable();
    await command.fill('java -jar picked.jar');
    await expect(page.getByLabel('Startup command')).toHaveValue('');
  });

  test('picks an egg image or a custom one', async ({ page }) => {
    await gotoReady(page, '/servers/create');
    await fillGeneral(page);
    await next(page);

    const image = page.getByLabel('Image', { exact: true });
    const select = page.getByLabel('Image name');
    await select.selectOption({ label: 'Java 17' });
    await expect(image).toHaveValue('ghcr.io/pelican-eggs/yolks:java_17');
    await expect(page.getByText('Custom image. Must be')).toBeHidden();

    await image.fill('registry.example/custom:1');
    await expect(select).toHaveValue('');
    await expect(page.getByText('Custom image. Must be')).toBeVisible();

    // Picking Custom image switches to custom mode and keeps the field.
    await select.selectOption({ label: 'Java 17' });
    await select.selectOption({ label: 'Custom image' });
    await expect(select).toHaveValue('');
    await expect(image).toHaveValue('ghcr.io/pelican-eggs/yolks:java_17');
    await expect(page.getByText('Custom image. Must be')).toBeVisible();
    await image.fill('');
    await next(page);
    await expect(page.getByText('Image is required.')).toBeVisible();
    await expect(select).toHaveValue('');
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
    await page.getByRole('button', { name: 'Create server' }).click();
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
    await page.getByRole('button', { name: 'Create server' }).click();
    await expect(page).toHaveURL(/\/servers\/srv-new$/);
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
    await page.getByRole('button', { name: 'Create server' }).click();

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
    await page.getByRole('button', { name: 'Create server' }).click();

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
    await page.getByRole('button', { name: 'Create server' }).click();

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
    await page.getByRole('button', { name: 'Create server' }).click();

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

test.describe('servers without server:edit', () => {
  test.beforeEach(async ({ context }) => setupSession(context, 'readonly'));

  test('list disables create and delete but not edit', async ({ page }) => {
    await gotoReady(page, '/servers');

    await expect(row(page, 'Survival SMP')).toBeVisible();
    await expect(
      page.getByRole('link', { name: 'Create server' })
    ).toHaveAttribute('aria-disabled', 'true');
    // Owners and write grantees edit without server:edit; the page enforces access.
    const edit = row(page, 'Survival SMP').getByRole('link').last();
    await expect(edit).not.toHaveAttribute('aria-disabled', 'true');
    await expect(edit).toHaveAttribute('href', '/servers/srv-survival');
    await expect(
      row(page, 'Survival SMP').getByRole('button').last()
    ).toBeDisabled();
    // The name still links to the (read-only) server page.
    await expect(
      row(page, 'Survival SMP').getByRole('link', { name: 'Survival SMP' })
    ).toHaveAttribute('href', '/servers/srv-survival');
  });
});

// ---------------------------------------------------------------------------
// Edit page
// ---------------------------------------------------------------------------

const tab = (page: Page, name: string) =>
  page.getByRole('tab', { name: new RegExp(`^${name}`) });

const openTab = async (page: Page, name: string) => tab(page, name).click();

const save = async (page: Page) =>
  page.getByRole('button', { name: 'Save Changes' }).click();

/** Serves `serverInfo` with `access` for every server. */
const asAccess = (access: 'read' | 'write' | 'admin') =>
  gen.handleServerInfo(
    ({ params }) =>
      HttpResponse.json(serverInfoOf(params.uuid, access)) as never
  );

/** Records `updateServer` bodies and answers with `respond` (default 200). */
const recordUpdate = (respond: () => never = ok) => {
  const bodies: unknown[] = [];
  return {
    bodies,
    handler: gen.handleUpdateServer(async ({ request }) => {
      bodies.push(await request.json());
      return respond();
    })
  };
};

/** `PUT` body of srv-survival saved unchanged by an admin. */
const survivalBody = {
  allocations: [
    { ip: '10.0.0.1', port: 25_565 },
    { ip: '10.0.0.1', port: 25_566 },
    { ip: '10.0.0.1', port: 25_567 }
  ],
  backup_limit: 2,
  egg: 'egg-paper',
  environment: {
    MAX_PLAYERS: '20',
    SERVER_JARFILE: 'server.jar',
    VERSION: 'latest'
  },
  grants: {
    groups: [{ access: 'read', uuid: 'group-staff' }],
    users: [{ access: 'write', uuid: 'user-1' }]
  },
  image: 'ghcr.io/pelican-eggs/yolks:java_21',
  limits: { cpu: 2000, disk: 0, memory: 4096 },
  name: 'Survival SMP',
  owner: 'user-admin',
  startup: 'java -jar {{SERVER_JARFILE}}',
  startup_overridden: false
};

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

  test('shows a skeleton while loading', async ({ network, page }) => {
    network.use(
      gen.handleServerInfo(async () => {
        await delay('infinite');
        return ok();
      })
    );
    await page.goto('/servers/srv-survival');

    await expect(page.locator('[aria-busy=true]')).toBeVisible();
    await expect(page.getByRole('tab')).toHaveCount(0);
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
    await page
      .getByLabel('Startup command')
      .selectOption({ label: 'Aikar flags' });
    await expect(page.getByText('Overridden')).toBeVisible();
    await page.getByLabel('Image name').selectOption({ label: 'Java 17' });

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

const action = (page: Page, name: string) =>
  page.getByRole('button', { exact: true, name });

test.describe('server status actions', () => {
  test.beforeEach(async ({ context }) => setupSession(context));

  for (const { alert, name, off, on, uuid } of matrix) {
    test(`${name} enables the actions its status allows`, async ({ page }) => {
      await gotoReady(page, `/servers/${uuid}`);
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
    await gotoReady(page, '/servers/srv-survival');

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
    await gotoReady(page, '/servers/srv-survival');

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
    await gotoReady(page, '/servers/srv-fresh');
    await action(page, 'Mark as installed').click();
    await expect(
      page.getByText('Fresh Server marked as installed')
    ).toBeVisible();

    await gotoReady(page, '/servers/srv-survival');
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
    await gotoReady(page, '/servers/srv-fresh');
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
    await gotoReady(page, '/servers/srv-survival');
    await action(page, 'Suspend').click();
    const dialog = page.getByRole('dialog');
    await expect(dialog.getByText('Suspend Survival SMP?')).toBeVisible();
    await dialog.getByRole('button', { name: 'Suspend' }).click();
    await expect(page.getByText('Survival SMP suspended')).toBeVisible();

    await gotoReady(page, '/servers/srv-banned');
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
    await gotoReady(page, '/servers/srv-survival');
    await action(page, 'Suspend').click();
    await page
      .getByRole('dialog')
      .getByRole('button', { name: 'Suspend' })
      .click();
    await expect(page.getByText("Couldn't suspend server.")).toBeVisible();

    await gotoReady(page, '/servers/srv-banned');
    await action(page, 'Unsuspend').click();
    await expect(page.getByText("Couldn't unsuspend server.")).toBeVisible();
  });

  test('transfers to another node', async ({ network, page }) => {
    const bodies: unknown[] = [];
    network.use(
      gen.handleTransferServer(async ({ request }) => {
        bodies.push(await request.json());
        return ok();
      })
    );
    await gotoReady(page, '/servers/srv-survival');
    await action(page, 'Transfer…').click();
    const dialog = page.getByRole('dialog');
    await expect(dialog.getByText('Transfer Survival SMP')).toBeVisible();

    // Empty submit validates in the dialog.
    await dialog.getByRole('button', { name: 'Start transfer' }).click();
    await expect(dialog.getByText('Select a target node.')).toBeVisible();
    await expect(
      dialog.getByText('Enter a valid IPv4 or IPv6 address.')
    ).toBeVisible();

    // The current node is not a target.
    await combo(dialog, 'Target node').click();
    await expect(page.getByRole('option', { name: 'Node One' })).toHaveCount(0);
    await page.getByRole('option', { name: 'Node Two' }).click();
    await dialog.getByLabel('IP address').fill(' 10.0.0.2 ');
    await dialog.getByLabel('Port').fill('80');
    await dialog.getByRole('button', { name: 'Start transfer' }).click();
    await expect(
      dialog.getByText('Port must be between 1024 and 65535.')
    ).toBeVisible();

    await dialog.getByLabel('Port').fill('25600');
    await dialog.getByRole('button', { name: 'Start transfer' }).click();
    await expect(
      page.getByText('Server Survival SMP transferred to Node Two')
    ).toBeVisible();
    await expect(dialog).toBeHidden();
    expect(bodies).toEqual([{ ip: '10.0.0.2', node: 'node-2', port: 25_600 }]);
  });

  for (const { body, message, status } of [
    {
      body: { errors: [{ field: 'ip', message: 'IP is not on the node.' }] },
      message: "Couldn't transfer server. Fix the highlighted fields.",
      status: 400
    },
    {
      body: { errors: [{ field: 'port', message: 'Port is taken.' }] },
      message: "Couldn't transfer server: the allocation is already in use.",
      status: 409
    },
    {
      body: { errors: [{ field: 'node', message: 'Node is offline.' }] },
      message: "Couldn't transfer server. Fix the highlighted fields.",
      status: 400
    },
    {
      body: { errors: [{ field: 'other', message: 'Odd transfer.' }] },
      message: 'Odd transfer.',
      status: 400
    },
    { body: undefined, message: "Couldn't transfer server.", status: 500 }
  ]) {
    test(`shows a ${status} transfer error for ${body?.errors[0].field ?? 'no body'}`, async ({
      network,
      page
    }) => {
      network.use(
        gen.handleTransferServer(
          () =>
            (body
              ? HttpResponse.json(body, { status })
              : new HttpResponse(null, { status })) as never
        )
      );
      await gotoReady(page, '/servers/srv-survival');
      await action(page, 'Transfer…').click();
      const dialog = page.getByRole('dialog');
      await combo(dialog, 'Target node').click();
      await page.getByRole('option', { name: 'Node Two' }).click();
      await dialog.getByLabel('IP address').fill('10.0.0.2');
      await dialog.getByRole('button', { name: 'Start transfer' }).click();

      await expect(page.getByText(message).first()).toBeVisible();
      const inline = body?.errors[0];
      if (inline && inline.field !== 'other') {
        await expect(dialog.getByText(inline.message)).toBeVisible();
      }
      await expect(dialog).toBeVisible();
    });
  }

  test('disables transfer with a single node', async ({ network, page }) => {
    network.use(
      gen.handleServerOptions(
        () =>
          HttpResponse.json({
            ...serverOptions.default,
            nodes: [{ name: 'Node One', uuid: 'node-1' }]
          }) as never
      )
    );
    await gotoReady(page, '/servers/srv-survival');
    await expect(action(page, 'Reinstall')).toBeEnabled();
    await expect(action(page, 'Transfer…')).toBeDisabled();
  });

  for (const button of ['Delete', 'Delete server']) {
    test(`deletes from "${button}" and returns to the list`, async ({
      network,
      page
    }) => {
      const deleted: unknown[] = [];
      network.use(
        gen.handleDeleteServer(({ params }) => {
          deleted.push(params.uuid);
          return ok();
        })
      );
      await gotoReady(page, '/servers/srv-survival');
      await action(page, button).click();
      const dialog = page.getByRole('dialog');
      await expect(
        dialog.getByText('Delete server Survival SMP?')
      ).toBeVisible();
      await dialog.getByRole('button', { name: 'Delete' }).click();

      await expect(page).toHaveURL(/\/servers$/);
      await expect(
        page.getByText('Server Survival SMP deleted successfully')
      ).toBeVisible();
      expect(deleted).toEqual(['srv-survival']);
    });
  }

  test('shows a delete failure and stays', async ({ network, page }) => {
    network.use(
      gen.handleDeleteServer(
        () => new HttpResponse(null, { status: 500 }) as never
      )
    );
    await gotoReady(page, '/servers/srv-survival');
    await action(page, 'Delete server').click();
    await page
      .getByRole('dialog')
      .getByRole('button', { name: 'Delete' })
      .click();

    await expect(page.getByText('Failed to delete server')).toBeVisible();
    await expect(page).toHaveURL(/\/servers\/srv-survival$/);
  });
});

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
    await gotoReady(page, '/servers/srv-survival');

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
    await gotoReady(page, '/servers/srv-survival');

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
      await gotoReady(page, '/servers/srv-survival');
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
    await gotoReady(page, '/servers/srv-survival');
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
    await gotoReady(page, '/servers/srv-survival');
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

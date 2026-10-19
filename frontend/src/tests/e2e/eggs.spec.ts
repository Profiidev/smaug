import { readFile } from 'node:fs/promises';
import { type Page, expect } from '@playwright/test';
import { HttpResponse } from 'msw';
import * as gen from '$lib/client/msw.gen';
import { test } from '$test_helpers/e2e-fixture';
import { setupSession } from '$test_helpers/session';
import { expectNoHorizontalOverflow, gotoReady } from '$test_helpers/layout';
import { eggInfo } from '../mocks/e2e/data';

/** Records the JSON body of every request a handler sees. */
const recorder = () => {
  const bodies: Record<string, unknown>[] = [];
  return {
    bodies,
    record: async (request: Request) => {
      bodies.push((await request.json()) as Record<string, unknown>);
    }
  };
};

const openTab = async (page: Page, name: string) =>
  page.getByRole('tab', { name }).click();

const badRequest = (message: string) =>
  new HttpResponse(message, { status: 400 }) as never;

const plcnFile = {
  ...eggInfo,
  _comment: 'DO NOT EDIT: FILE GENERATED AUTOMATICALLY BY PANEL',
  config: {
    files: JSON.stringify(eggInfo.config.files),
    logs: '{}',
    startup: JSON.stringify(eggInfo.config.startup),
    stop: 'stop'
  },
  exported_at: '2026-01-01T00:00:00+00:00',
  meta: { update_url: null, version: 'PLCN_v3' },
  name: 'Imported Paper',
  uuid: '9eec81aa-b165-4233-aa30-1244c6c96e9f'
};

const ptdlFile = {
  _comment:
    'DO NOT EDIT: FILE GENERATED AUTOMATICALLY BY PTERODACTYL PANEL - PTERODACTYL.IO',
  author: 'dev@example.com',
  config: plcnFile.config,
  description: null,
  docker_images: eggInfo.docker_images,
  exported_at: '2024-01-01T00:00:00+00:00',
  features: [],
  file_denylist: [],
  meta: { update_url: null, version: 'PTDL_v2' },
  name: 'Legacy Paper',
  scripts: eggInfo.scripts,
  startup: 'java -jar server.jar',
  variables: []
};

const notSquashed = async (page: Page) =>
  page
    .locator('[data-slot="card"] [data-slot="card"]')
    .evaluateAll((cards) =>
      cards.every((el) => el.scrollHeight <= el.clientHeight + 1)
    );

const pageScrolls = async (page: Page) =>
  page.evaluate(
    () =>
      document.scrollingElement!.scrollHeight >
      document.scrollingElement!.clientHeight
  );

const next = async (page: Page) =>
  page.getByRole('button', { name: 'Next' }).click();

const importEgg = async (page: Page, egg: object = plcnFile) => {
  await page.getByLabel(/Import egg JSON/).setInputFiles({
    buffer: Buffer.from(JSON.stringify(egg)),
    mimeType: 'application/json',
    name: 'egg-paper.json'
  });
  await expect(page.getByText('Imported egg-paper.json')).toBeVisible();
};

/** Clicks through the remaining wizard stages and submits. */
const finishWizard = async (page: Page) => {
  // Each stage is confirmed by one of its own controls.
  for (const ready of [
    page.getByRole('button', { name: 'Add variable' }),
    page.getByLabel('Install Container Image'),
    page.getByLabel('Stop Command'),
    page.getByRole('button', { name: 'Add config file' })
  ]) {
    // oxlint-disable-next-line no-await-in-loop
    await next(page);
    // oxlint-disable-next-line no-await-in-loop
    await expect(ready).toBeVisible();
  }
  await page.getByRole('button', { name: 'Create Egg' }).click();
};

test.describe('eggs list', () => {
  test.beforeEach(async ({ context }) => setupSession(context));

  test('lists the eggs and fits the viewport', async ({ page }) => {
    await gotoReady(page, '/eggs');
    await expect(page.getByRole('heading', { name: 'Eggs' })).toBeVisible();
    await expect(
      page.getByRole('cell', { exact: true, name: 'Paper' })
    ).toBeVisible();
    await expect(
      page.getByRole('cell', { name: 'ops@example.com' })
    ).toBeVisible();
    await expect(
      page.getByText(
        'Server templates that define how game servers are installed and run.'
      )
    ).toBeVisible();
    await expect(page.getByRole('link', { name: 'Create egg' })).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Import JSON' })
    ).toBeVisible();
    await expect(page.getByText('2 eggs')).toBeVisible();
    await expectNoHorizontalOverflow(page);
  });

  test('links the egg name to its detail page', async ({ page }) => {
    await gotoReady(page, '/eggs');
    await page.getByRole('link', { exact: true, name: 'Paper' }).click();
    await expect(page).toHaveURL(/\/eggs\/egg-paper$/);
    await expect(page.getByLabel('Egg Name')).toHaveValue('Paper');
  });

  test('shows the empty state with create and import', async ({
    context,
    page
  }) => {
    await setupSession(context, 'empty');
    await gotoReady(page, '/eggs');
    await expect(page.getByText('No eggs yet')).toBeVisible();
    await expect(page.getByRole('table')).toHaveCount(0);
    await expect(page.getByRole('link', { name: 'Create egg' })).toHaveCount(1);
    await expect(page.getByRole('button', { name: 'Import JSON' })).toHaveCount(
      1
    );
    await expectNoHorizontalOverflow(page);
    await page.getByRole('link', { name: 'Create egg' }).click();
    await expect(page).toHaveURL(/\/eggs\/create$/);
  });

  test('imports from the list into the create wizard', async ({ page }) => {
    await gotoReady(page, '/eggs');
    await page.getByLabel(/Import egg JSON/).setInputFiles({
      buffer: Buffer.from(JSON.stringify(ptdlFile)),
      mimeType: 'application/json',
      name: 'egg-legacy.json'
    });
    await expect(page).toHaveURL(/\/eggs\/create$/);
    await expect(page.getByText('Imported from PTDL_v2')).toBeVisible();
    await expect(
      page.getByText(
        'Upgraded to PLCN_v3. Review every step before creating the egg.'
      )
    ).toBeVisible();
    await expect(page.getByLabel('Egg Name')).toHaveValue('Legacy Paper');
    await expectNoHorizontalOverflow(page);
  });

  test('rejects a bad file on the list without navigating', async ({
    page
  }) => {
    await gotoReady(page, '/eggs');
    await page.getByLabel(/Import egg JSON/).setInputFiles({
      buffer: Buffer.from('{nope'),
      mimeType: 'application/json',
      name: 'bad.json'
    });
    await expect(page.getByText('Invalid egg file')).toBeVisible();
    await expect(page).toHaveURL(/\/eggs$/);
  });

  test('deletes an egg from the list', async ({ network, page }) => {
    const del = recorder();
    network.use(
      gen.handleDeleteEgg(async ({ request }) => {
        await del.record(request);
        return new HttpResponse(null, { status: 200 }) as never;
      })
    );
    await gotoReady(page, '/eggs');
    await page
      .getByRole('row', { name: /Valheim/ })
      .getByRole('button')
      .last()
      .click();
    const dialog = page.getByRole('dialog');
    await expect(dialog.getByText('Delete egg Valheim?')).toBeVisible();
    await expect(
      dialog.getByText(
        'This cannot be undone. The egg and its variables, scripts and config files are removed.'
      )
    ).toBeVisible();
    await dialog.getByRole('button', { name: 'Delete' }).click();
    await expect(
      page.getByText('Egg Valheim deleted successfully')
    ).toBeVisible();
    expect(del.bodies).toEqual([{ uuid: 'egg-valheim' }]);
  });

  test('shows the server error when delete fails', async ({
    network,
    page
  }) => {
    network.use(gen.handleDeleteEgg(() => badRequest('Egg is in use')));
    await gotoReady(page, '/eggs');
    await page
      .getByRole('row', { name: /Valheim/ })
      .getByRole('button')
      .last()
      .click();
    await page
      .getByRole('dialog')
      .getByRole('button', { name: 'Delete' })
      .click();
    await expect(page.getByText('Egg is in use')).toBeVisible();
  });

  test('redirects an unknown egg to the list with a toast', async ({
    page
  }) => {
    await gotoReady(page, '/eggs/egg-missing');
    await expect(page).toHaveURL(/\/eggs$/);
    await expect(page.getByText('Egg not found')).toBeVisible();
  });

  test('redirects other load failures with a generic toast', async ({
    network,
    page
  }) => {
    network.use(
      gen.handleEggInfo(() => new HttpResponse(null, { status: 500 }) as never)
    );
    await gotoReady(page, '/eggs/egg-paper');
    await expect(page).toHaveURL(/\/eggs$/);
    await expect(page.getByText('Failed to load egg')).toBeVisible();
  });
});

test.describe('egg create wizard', () => {
  test.beforeEach(async ({ context }) => setupSession(context));

  test('only the stage fields scroll, cards keep their height', async ({
    page
  }) => {
    await page.setViewportSize({ height: 600, width: 1280 });
    await gotoReady(page, '/eggs/create');
    await page.getByLabel('Egg Name').fill('My Egg');
    await page.getByLabel('Author Email').fill('me@example.com');
    await next(page);
    const add = page.getByRole('button', { name: 'Add variable' });
    await add.click();
    await add.click();
    await add.click();
    expect(await notSquashed(page)).toBe(true);
    expect(await pageScrolls(page)).toBe(false);
    await expect(page.getByRole('button', { name: 'Next' })).toBeInViewport();
    await expect(page.getByText('Variables', { exact: true })).toBeInViewport();
  });

  test('runtime lists label their columns once', async ({ page }) => {
    await gotoReady(page, '/eggs/create');
    await page.getByLabel('Egg Name').fill('My Egg');
    await page.getByLabel('Author Email').fill('me@example.com');
    await next(page);
    await next(page);
    await page
      .getByLabel('Install Container Image')
      .fill('ghcr.io/installers:debian');
    await next(page);
    await page.getByRole('button', { name: 'Add image' }).click();
    // One header per list, no per-row <label>, every input still named.
    await expect(page.getByText('Image', { exact: true })).toHaveCount(1);
    await expect(page.getByText('Command', { exact: true })).toHaveCount(1);
    await expect(page.locator('label', { hasText: /^Image$/u })).toHaveCount(0);
    await expect(page.getByLabel('Image', { exact: true })).toHaveCount(2);
    await expect(page.getByLabel('Command', { exact: true })).toHaveCount(1);
    // Rows have no empty label, and input, arrow and remove button line up.
    const row = page
      .getByLabel('Image', { exact: true })
      .first()
      .locator('xpath=ancestor::div[contains(@class,"items-start")][1]');
    await expect(row.locator('label')).toHaveCount(0);
    // Each visible arrow/button is centred on the input sharing its line (rows wrap on mobile).
    const offsets = await row.evaluate((el) => {
      const mid = (e: Element) => {
        const b = e.getBoundingClientRect();
        return b.height ? b.top + b.height / 2 : undefined;
      };
      const inputs = [...el.querySelectorAll('input')].map(mid);
      return [el.querySelector('svg')!, el.querySelector('button')!]
        .map(mid)
        .filter((c) => c !== undefined)
        .map((c) => Math.min(...inputs.map((m) => Math.abs(m! - c))));
    });
    expect(offsets.length).toBeGreaterThan(0);
    for (const offset of offsets) {
      expect(offset).toBeLessThanOrEqual(1);
    }
  });

  test('creates an egg with nested variables and docker images', async ({
    network,
    page
  }) => {
    const create = recorder();
    network.use(
      gen.handleCreateEgg(async ({ request }) => {
        await create.record(request);
        return HttpResponse.json({ uuid: 'egg-new' }) as never;
      })
    );
    await gotoReady(page, '/eggs/create');

    // General: validation keeps the wizard on the stage.
    await next(page);
    await expect(page.getByText('Egg name is required')).toBeVisible();
    await page.getByLabel('Egg Name').fill('My Egg');
    await page.getByLabel('Author Email').fill('me@example.com');
    await next(page);

    // Variables: two variables with nested fields.
    await page.getByRole('button', { name: 'Add variable' }).click();
    await page.getByRole('button', { name: 'Add variable' }).click();
    await expect(page.getByText('Startup variables (2)')).toBeVisible();
    for (const [i, env] of ['FIRST', 'SECOND'].entries()) {
      // oxlint-disable-next-line no-await-in-loop
      await page
        .getByLabel('Name', { exact: true })
        .nth(i)
        .fill(`Var ${i + 1}`);
      // oxlint-disable-next-line no-await-in-loop
      await page.getByLabel('Env Variable').nth(i).fill(env);
      // oxlint-disable-next-line no-await-in-loop
      await page
        .getByLabel('Default Value')
        .nth(i)
        .fill(`value-${i + 1}`);
    }
    await page.getByRole('switch', { name: 'Visible to users' }).nth(1).click();
    await next(page);

    // Install script.
    await page.getByLabel('Install Container Image').fill('alpine:3');
    await next(page);

    // Runtime: docker image, startup command, done string, stop.
    await page.getByLabel('Label', { exact: true }).first().fill('Alpine');
    await page
      .getByLabel('Image', { exact: true })
      .fill('ghcr.io/yolks:alpine');
    await page.getByLabel('Command', { exact: true }).fill('./start.sh');
    const done = page.getByPlaceholder(/Substring, or regex/);
    await done.fill('Server started');
    await done.press('Enter');
    await page.getByLabel('Stop Command').fill('stop');
    await next(page);

    await expect(
      page.getByRole('button', { name: 'Add config file' })
    ).toBeVisible();
    await page.getByRole('button', { name: 'Create Egg' }).click();

    await expect(page.getByText('Egg created successfully.')).toBeVisible();
    await expect(page).toHaveURL(/\/eggs\/egg-new$/);

    expect(create.bodies).toHaveLength(1);
    const [body] = create.bodies;
    expect(body).not.toHaveProperty('_comment');
    expect(body).not.toHaveProperty('uuid');
    expect(body).toMatchObject({
      author: 'me@example.com',
      docker_images: { Alpine: 'ghcr.io/yolks:alpine' },
      name: 'My Egg',
      scripts: { installation: { container: 'alpine:3', entrypoint: 'bash' } },
      startup_commands: { Default: './start.sh' },
      variables: [
        {
          default_value: 'value-1',
          env_variable: 'FIRST',
          name: 'Var 1',
          rules: ['required', 'string'],
          sort: 1,
          user_viewable: false
        },
        {
          default_value: 'value-2',
          env_variable: 'SECOND',
          name: 'Var 2',
          rules: ['required', 'string'],
          sort: 2,
          user_viewable: true
        }
      ]
    });
    const config = body.config as Record<string, string>;
    expect(JSON.parse(config.startup)).toEqual({ done: 'Server started' });
    expect(config.stop).toBe('stop');
  });

  test('rejects reserved and duplicate env variables before submit', async ({
    page
  }) => {
    await gotoReady(page, '/eggs/create');
    await page.getByLabel('Egg Name').fill('My Egg');
    await page.getByLabel('Author Email').fill('me@example.com');
    await next(page);
    await page.getByRole('button', { name: 'Add variable' }).click();
    await page.getByRole('button', { name: 'Add variable' }).click();
    await page.getByLabel('Name', { exact: true }).nth(0).fill('A');
    await page.getByLabel('Env Variable').nth(0).fill('SERVER_IP');
    await page.getByLabel('Name', { exact: true }).nth(1).fill('B');
    await page.getByLabel('Env Variable').nth(1).fill('SERVER_IP');
    await next(page);
    await expect(
      page.getByText('Reserved name — choose another.').first()
    ).toBeVisible();
    await page.getByLabel('Env Variable').nth(0).fill('DUP');
    await page.getByLabel('Env Variable').nth(1).fill('DUP');
    await next(page);
    await expect(page.getByText('Duplicate env variable name.')).toBeVisible();
    await expect(page.getByText('Startup variables (2)')).toBeVisible();
  });

  test('imports a PLCN_v3 file and creates it', async ({ network, page }) => {
    const create = recorder();
    network.use(
      gen.handleCreateEgg(async ({ request }) => {
        await create.record(request);
        return HttpResponse.json({ uuid: 'egg-imported' }) as never;
      })
    );
    await gotoReady(page, '/eggs/create');
    await expect(page.getByText(/Imported from/)).toHaveCount(0);
    await importEgg(page);
    await expect(page.getByText('Imported from PLCN_v3')).toBeVisible();
    await expect(page.getByLabel('Egg Name')).toHaveValue('Imported Paper');
    await expect(page.getByLabel('Author Email')).toHaveValue(
      'dev@example.com'
    );
    await finishWizard(page);

    await expect(page).toHaveURL(/\/eggs\/egg-imported$/);
    expect(create.bodies[0]).toMatchObject({
      docker_images: eggInfo.docker_images,
      name: 'Imported Paper',
      startup_commands: eggInfo.startup_commands,
      variables: [{ env_variable: 'SERVER_JARFILE', sort: 1 }]
    });
  });

  test('shows a description per stage and fits the viewport', async ({
    page
  }) => {
    await gotoReady(page, '/eggs/create');
    await expect(
      page.getByText('Basic details, icon and feature flags.')
    ).toBeVisible();
    await expectNoHorizontalOverflow(page);
    await importEgg(page);
    for (const text of [
      'Environment variables users can configure on a server.',
      'The container and script that install the server files.',
      'Docker images, startup commands and how the panel detects a running server.',
      'Files the panel edits before each start.'
    ]) {
      // oxlint-disable-next-line no-await-in-loop
      await next(page);
      // oxlint-disable-next-line no-await-in-loop
      await expect(page.getByText(text)).toBeVisible();
      // oxlint-disable-next-line no-await-in-loop
      await expectNoHorizontalOverflow(page);
    }
  });

  test('edits find entries with type-specific controls', async ({
    network,
    page
  }) => {
    const create = recorder();
    network.use(
      gen.handleCreateEgg(async ({ request }) => {
        await create.record(request);
        return HttpResponse.json({ uuid: 'egg-new' }) as never;
      })
    );
    await gotoReady(page, '/eggs/create');
    await importEgg(page);
    for (let i = 0; i < 4; i += 1) {
      // oxlint-disable-next-line no-await-in-loop
      await next(page);
    }
    await page.getByRole('button', { name: 'Add config file' }).click();
    // Each config file card is the Collapsible root; the new one is last.
    const card = page.locator('[data-slot=collapsible]').last();
    await expect(card.getByText('New config file')).toBeVisible();
    await card.getByLabel('File Path').fill('config.yml');
    await expect(card.getByText('config.yml', { exact: true })).toBeVisible();
    await expect(card.getByText('0 find entries')).toBeVisible();

    const add = card.getByRole('button', { name: 'Add find entry' });
    await add.click();
    await card.getByLabel('Key').nth(0).fill('port');
    await card.getByRole('radio', { name: 'Number' }).nth(0).click();
    await expect(card.getByLabel('Value')).toHaveAttribute('type', 'number');
    await card.getByLabel('Value').fill('25565');

    await add.click();
    await card.getByLabel('Key').nth(1).fill('online');
    await card.getByRole('radio', { name: 'Boolean' }).nth(1).click();
    await card.getByLabel('Value').nth(1).click();
    await page.getByRole('option', { name: 'true' }).click();

    await add.click();
    await card.getByLabel('Key').nth(2).fill('ip');
    await card.getByRole('radio', { name: 'Map' }).nth(2).click();
    await card.getByRole('button', { name: 'Add pair' }).click();
    await card.getByLabel('Match').fill('0.0.0.0');
    await card
      .getByLabel('Replace with')
      .fill('{{server.allocations.default.ip}}');
    await expect(card.getByText('3 find entries')).toBeVisible();

    // Collapsing hides the body but keeps the header summary.
    await card.getByRole('button', { name: 'Toggle config file' }).click();
    await expect(card.getByLabel('File Path')).toBeHidden();
    await expect(card.getByText('3 find entries')).toBeVisible();

    await page.getByRole('button', { name: 'Create Egg' }).click();
    await expect(page).toHaveURL(/\/eggs\/egg-new$/);
    const config = create.bodies[0].config as Record<string, string>;
    expect(JSON.parse(config.files)['config.yml']).toEqual({
      find: {
        ip: { '0.0.0.0': '{{server.allocations.default.ip}}' },
        online: true,
        port: 25_565
      },
      parser: 'file'
    });
  });

  test('rejects an unknown file format', async ({ page }) => {
    await gotoReady(page, '/eggs/create');
    await page.getByLabel(/Import egg JSON/).setInputFiles({
      buffer: Buffer.from(JSON.stringify({ meta: { version: 'NOPE' } })),
      mimeType: 'application/json',
      name: 'bad.json'
    });
    await expect(
      page.getByText('The file format is not recognized.')
    ).toBeVisible();
    await expect(page.getByLabel('Egg Name')).toHaveValue('');
  });

  test('shows the server 400 message', async ({ network, page }) => {
    network.use(
      gen.handleCreateEgg(() =>
        badRequest("Environment variable 'TZ' is reserved")
      )
    );
    await gotoReady(page, '/eggs/create');
    await importEgg(page);
    await finishWizard(page);
    await expect(
      page.getByText("Environment variable 'TZ' is reserved")
    ).toBeVisible();
    await expect(page).toHaveURL(/\/eggs\/create$/);
    await expect(page.getByRole('button', { name: 'Retry' })).toBeVisible();
  });

  test('falls back to a generic message without a body', async ({
    network,
    page
  }) => {
    network.use(
      gen.handleCreateEgg(
        () => new HttpResponse(null, { status: 500 }) as never
      )
    );
    await gotoReady(page, '/eggs/create');
    await importEgg(page);
    await finishWizard(page);
    await expect(page.getByText('Error creating egg.')).toBeVisible();
  });
});

test.describe('egg detail', () => {
  test.beforeEach(async ({ context }) => setupSession(context));

  test('loads the egg into the form', async ({ page }) => {
    await gotoReady(page, '/eggs/egg-paper');
    await expect(
      page.getByRole('heading', { name: 'Egg: Paper' })
    ).toBeVisible();
    await expect(page.getByLabel('Egg Name')).toHaveValue('Paper');
    await expect(page.getByLabel('Env Variable')).toHaveValue('SERVER_JARFILE');
    await expect(page.getByLabel('File Path')).toHaveValue('server.properties');
    await expect(page.getByLabel('Stop Command')).toHaveValue('stop');
    await expectNoHorizontalOverflow(page);
  });

  test('saves edits, including nested fields', async ({ network, page }) => {
    const edit = recorder();
    network.use(
      gen.handleEditEgg(async ({ request }) => {
        await edit.record(request);
        return new HttpResponse(null, { status: 200 }) as never;
      })
    );
    await gotoReady(page, '/eggs/egg-paper');
    await expect(page.getByLabel('Egg Name')).toHaveValue('Paper');
    await page.getByLabel('Egg Name').fill('Paper 2');
    await openTab(page, 'Variables');
    await page.getByLabel('Default Value').fill('paper.jar');
    await openTab(page, 'Runtime');
    await page.getByRole('button', { name: 'Add image' }).click();
    await page.getByLabel('Label', { exact: true }).nth(1).fill('Java 17');
    await page
      .getByLabel('Image', { exact: true })
      .nth(1)
      .fill('ghcr.io/yolks:java_17');
    await page.getByRole('button', { name: 'Save Changes' }).click();

    await expect(
      page.getByText('Egg Paper 2 updated successfully')
    ).toBeVisible();
    await expect(
      page.getByRole('heading', { name: 'Egg: Paper 2' })
    ).toBeVisible();
    expect(edit.bodies).toHaveLength(1);
    expect(edit.bodies[0]).toMatchObject({
      docker_images: {
        'Java 17': 'ghcr.io/yolks:java_17',
        'Java 21': 'ghcr.io/pelican-eggs/yolks:java_21'
      },
      name: 'Paper 2',
      uuid: 'egg-paper',
      variables: [{ default_value: 'paper.jar', sort: 1 }]
    });
    // Form keeps the edited values (no reset).
    await openTab(page, 'General');
    await expect(page.getByLabel('Egg Name')).toHaveValue('Paper 2');
  });

  test('sticky save bar tracks unsaved changes', async ({ network, page }) => {
    network.use(
      gen.handleEditEgg(() => new HttpResponse(null, { status: 200 }) as never)
    );
    await gotoReady(page, '/eggs/egg-paper');
    await expect(page.getByLabel('Egg Name')).toHaveValue('Paper');
    await expect(page.getByText('Unsaved changes')).toHaveCount(0);
    const save = page.getByRole('button', { name: 'Save Changes' });
    // The bar stays in view while the top of the form is on screen.
    await expect(save).toBeInViewport();
    await page.getByLabel('Egg Name').fill('Paper 3');
    await expect(page.getByText('Unsaved changes')).toBeVisible();
    await save.click();
    await expect(
      page.getByText('Egg Paper 3 updated successfully')
    ).toBeVisible();
    await expect(page.getByText('Unsaved changes')).toHaveCount(0);
    await expect(page.getByLabel('Egg Name')).toHaveValue('Paper 3');
    await page.getByLabel('Egg Name').fill('Paper 4');
    await expect(page.getByText('Unsaved changes')).toBeVisible();
  });

  test('shows the server 400 message on save', async ({ network, page }) => {
    network.use(
      gen.handleEditEgg(() => badRequest('Startup commands must be distinct'))
    );
    await gotoReady(page, '/eggs/egg-paper');
    await expect(page.getByLabel('Egg Name')).toHaveValue('Paper');
    await page.getByRole('button', { name: 'Save Changes' }).click();
    await expect(
      page.getByText('Startup commands must be distinct')
    ).toBeVisible();
    await expect(page.getByRole('button', { name: 'Retry' })).toBeVisible();
  });

  test('blocks invalid edits client-side', async ({ network, page }) => {
    const edit = recorder();
    network.use(
      gen.handleEditEgg(async ({ request }) => {
        await edit.record(request);
        return new HttpResponse(null, { status: 200 }) as never;
      })
    );
    await gotoReady(page, '/eggs/egg-paper');
    await expect(page.getByLabel('Egg Name')).toHaveValue('Paper');
    await openTab(page, 'Variables');
    await page.getByLabel('Env Variable').fill('HOME');
    await openTab(page, 'General');
    await page.getByRole('button', { name: 'Save Changes' }).click();
    // The invalid field is on another tab: its trigger gets an error dot.
    const variablesTab = page.getByRole('tab', { name: 'Variables' });
    await expect(variablesTab.getByLabel('Has errors')).toBeVisible();
    await expect(
      page.getByRole('tab', { name: 'General' }).getByLabel('Has errors')
    ).toHaveCount(0);
    await variablesTab.click();
    await expect(
      page.getByText('Reserved name — choose another.')
    ).toBeVisible();
    expect(edit.bodies).toEqual([]);
  });

  test('only the section card scrolls, not the page or tab list', async ({
    page
  }) => {
    await page.setViewportSize({ height: 600, width: 1280 });
    await gotoReady(page, '/eggs/egg-paper');
    await expect(page.getByLabel('Egg Name')).toBeVisible();
    const tabList = page.getByRole('tablist');
    // Triggers' hidden underline pokes below the list; it must not become a scrollbar.
    expect(
      await tabList.evaluate(
        (el) =>
          getComputedStyle(el).overflowY === 'hidden' ||
          el.scrollHeight <= el.clientHeight
      )
    ).toBe(true);
    const content = page.locator(
      '[data-slot="tabs-content"][data-state="active"] [data-slot="card-content"]'
    );
    expect(
      await content.evaluate((el) => el.scrollHeight > el.clientHeight)
    ).toBe(true);
    await content.evaluate((el) => el.scrollTo(0, el.scrollHeight));
    expect(await pageScrolls(page)).toBe(false);
    await openTab(page, 'Variables');
    await page.getByRole('button', { name: 'Add variable' }).click();
    expect(await notSquashed(page)).toBe(true);
    expect(await pageScrolls(page)).toBe(false);
    await expect(page.getByRole('tab', { name: 'General' })).toBeInViewport();
    await expect(
      page.getByRole('button', { name: 'Save Changes' })
    ).toBeInViewport();
  });

  test('shows one section per tab', async ({ page }) => {
    await gotoReady(page, '/eggs/egg-paper');
    await expect(page.getByLabel('Egg Name')).toBeVisible();
    await expect(page.getByLabel('Env Variable')).toBeHidden();
    await openTab(page, 'Variables');
    await expect(page.getByLabel('Env Variable')).toBeVisible();
    await expect(page.getByLabel('Egg Name')).toBeHidden();
    await expectNoHorizontalOverflow(page);
  });

  test('export menu items stay on one line', async ({ page }) => {
    await gotoReady(page, '/eggs/egg-paper');
    await page.getByRole('button', { name: 'Export' }).click();
    const lines = await Promise.all(
      ['Pelican (PLCN_v3)', 'Pterodactyl (PTDL_v2)'].map(async (name) =>
        page
          .getByRole('menuitem', { name })
          .evaluate(
            (el) =>
              el.getBoundingClientRect().height /
              Number.parseFloat(getComputedStyle(el).lineHeight)
          )
      )
    );
    for (const count of lines) {
      expect(count).toBeLessThan(2);
    }
  });

  for (const [item, version, file] of [
    ['Pelican (PLCN_v3)', 'PLCN_v3', 'egg-paper.json'],
    ['Pterodactyl (PTDL_v2)', 'PTDL_v2', 'egg-paper.json']
  ] as const) {
    test(`exports ${version} from the form values`, async ({ page }) => {
      await gotoReady(page, '/eggs/egg-paper');
      await expect(page.getByLabel('Egg Name')).toHaveValue('Paper');
      await page.getByLabel('Egg Name').fill('Paper Edited');
      await page.getByRole('button', { name: 'Export' }).click();
      const [download] = await Promise.all([
        page.waitForEvent('download'),
        page.getByRole('menuitem', { name: item }).click()
      ]);
      expect(download.suggestedFilename()).toBe(
        file.replace('paper', 'paper-edited')
      );
      const egg = JSON.parse(await readFile(await download.path(), 'utf8'));
      expect(egg.meta.version).toBe(version);
      expect(egg.name).toBe('Paper Edited');
      if (version === 'PLCN_v3') {
        expect(egg.uuid).toBe('egg-paper');
        expect(egg.startup_commands).toEqual(eggInfo.startup_commands);
        expect(egg.variables[0].rules).toEqual(['required', 'string']);
      } else {
        expect(egg).not.toHaveProperty('uuid');
        expect(egg.startup).toBe('java -jar {{SERVER_JARFILE}}');
        expect(egg.variables[0].rules).toBe('required|string');
      }
    });
  }

  test('deletes the egg and returns to the list', async ({ network, page }) => {
    const del = recorder();
    network.use(
      gen.handleDeleteEgg(async ({ request }) => {
        await del.record(request);
        return new HttpResponse(null, { status: 200 }) as never;
      })
    );
    await gotoReady(page, '/eggs/egg-paper');
    await expect(
      page.getByRole('heading', { name: 'Egg: Paper' })
    ).toBeVisible();
    await page.getByRole('button', { name: 'Delete' }).click();
    await page
      .getByRole('dialog')
      .getByRole('button', { name: 'Delete' })
      .click();
    await expect(
      page.getByText('Egg Paper deleted successfully')
    ).toBeVisible();
    await expect(page).toHaveURL(/\/eggs$/);
    expect(del.bodies).toEqual([{ uuid: 'egg-paper' }]);
  });
});

test.describe('eggs without egg:edit', () => {
  test.beforeEach(async ({ context }) => setupSession(context, 'readonly'));

  test('list disables create, edit and delete', async ({ page }) => {
    await gotoReady(page, '/eggs');
    await expect(
      page.getByRole('cell', { exact: true, name: 'Paper' })
    ).toBeVisible();
    await expect(
      page.getByRole('link', { name: 'Create egg' })
    ).toHaveAttribute('aria-disabled', 'true');
    await expect(
      page.getByRole('row', { name: /Paper/ }).getByRole('button').last()
    ).toBeDisabled();
  });

  test('detail is read-only but can still export', async ({ page }) => {
    await gotoReady(page, '/eggs/egg-paper');
    await expect(page.getByLabel('Egg Name')).toHaveValue('Paper');
    await expect(page.getByLabel('Egg Name')).toBeDisabled();
    await expect(
      page.getByRole('switch', { name: 'EULA prompt' })
    ).toBeDisabled();
    await expect(
      page.getByRole('button', { name: 'Save Changes' })
    ).toBeDisabled();
    await openTab(page, 'Variables');
    await expect(page.getByLabel('Env Variable')).toBeDisabled();
    await expect(
      page.getByRole('button', { name: 'Add variable' })
    ).toBeDisabled();
    await openTab(page, 'Config');
    await expect(
      page.getByRole('radio', { name: 'Number' }).first()
    ).toBeDisabled();
    await expect(page.getByRole('button', { name: 'Delete' })).toBeDisabled();
    await page.getByRole('button', { name: 'Export' }).click();
    const [download] = await Promise.all([
      page.waitForEvent('download'),
      page.getByRole('menuitem', { name: 'Pelican (PLCN_v3)' }).click()
    ]);
    expect(download.suggestedFilename()).toBe('egg-paper.json');
  });
});

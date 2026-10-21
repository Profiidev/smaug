import { expect } from '@playwright/test';
import { HttpResponse } from 'msw';
import * as gen from '$lib/client/msw.gen';
import { test } from '$test_helpers/e2e-fixture';
import { setupSession } from '$test_helpers/session';
import { expectNoHorizontalOverflow, gotoReady } from '$test_helpers/layout';
import {
  badRequest,
  finishWizard,
  importEgg,
  next,
  notSquashed,
  pageScrolls,
  recorder
} from '$test_helpers/eggs-e2e';
import { submitWith } from '$test_helpers/servers-e2e';
import { eggInfo } from '../mocks/e2e/data';

test.describe('egg create wizard', () => {
  test.beforeEach(async ({ context }) => setupSession(context));

  test('keeps the header right above the wizard card', async ({ page }) => {
    await gotoReady(page, '/eggs/create');
    await expect(
      page.getByRole('heading', { name: 'Create egg' })
    ).toBeVisible();
    const header = (await page
      .getByRole('link', { name: 'Back to eggs' })
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
    await page.getByRole('button', { exact: true, name: 'Create' }).click();

    await expect(page.getByText('Egg created successfully.')).toBeVisible();
    await expect(page).toHaveURL(/\/eggs\/egg-new\/general$/);

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
    await submitWith(
      page,
      async () => {
        await page.getByLabel('Env Variable').nth(0).fill('DUP');
        await page.getByLabel('Env Variable').nth(1).fill('DUP');
      },
      page.getByText('Duplicate env variable name.')
    );
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

    await expect(page).toHaveURL(/\/eggs\/egg-imported\/general$/);
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

    await page.getByRole('button', { exact: true, name: 'Create' }).click();
    await expect(page).toHaveURL(/\/eggs\/egg-new\/general$/);
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

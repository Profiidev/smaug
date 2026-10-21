import { readFile } from 'node:fs/promises';
import { expect } from '@playwright/test';
import { HttpResponse } from 'msw';
import * as gen from '$lib/client/msw.gen';
import { test } from '$test_helpers/e2e-fixture';
import { setupSession } from '$test_helpers/session';
import { expectNoHorizontalOverflow, gotoReady } from '$test_helpers/layout';
import {
  applyUpdate,
  badRequest,
  changedSource,
  notSquashed,
  openTab,
  pageScrolls,
  recorder,
  sourceHandler
} from '$test_helpers/eggs-e2e';
import { eggInfo } from '../mocks/e2e/data';

test.describe('egg detail', () => {
  test.beforeEach(async ({ context }) => setupSession(context));

  test('loads the egg into the form', async ({ page }) => {
    await gotoReady(page, '/eggs/egg-paper/general');
    await expect(
      page.getByRole('heading', { name: 'Egg: Paper' })
    ).toBeVisible();
    await expect(page.getByLabel('Egg Name')).toHaveValue('Paper');
    await openTab(page, 'Variables');
    await page.getByRole('button', { name: 'Toggle variable' }).click();
    await expect(page.getByLabel('Env Variable')).toHaveValue('SERVER_JARFILE');
    await openTab(page, 'Config');
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
    await gotoReady(page, '/eggs/egg-paper/general');
    await expect(page.getByLabel('Egg Name')).toHaveValue('Paper');
    await page.getByLabel('Egg Name').fill('Paper 2');
    await openTab(page, 'Variables');
    await page.getByRole('button', { name: 'Toggle variable' }).click();
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
    await gotoReady(page, '/eggs/egg-paper/general');
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
    await gotoReady(page, '/eggs/egg-paper/general');
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
    await gotoReady(page, '/eggs/egg-paper/general');
    await expect(page.getByLabel('Egg Name')).toHaveValue('Paper');
    await openTab(page, 'Variables');
    const toggle = page.getByRole('button', { name: 'Toggle variable' });
    await toggle.click();
    await page.getByLabel('Env Variable').fill('HOME');
    // Collapse it again: the error must expand it so it is not hidden.
    await toggle.click();
    await expect(toggle).toHaveAttribute('aria-expanded', 'false');
    await openTab(page, 'General');
    await page.getByRole('button', { name: 'Save Changes' }).click();
    // The invalid field is on another tab: its trigger gets an error dot.
    const variablesTab = page.getByRole('tab', { name: 'Variables' });
    await expect(variablesTab.getByLabel('Has errors')).toBeVisible();
    await expect(
      page.getByRole('tab', { name: 'General' }).getByLabel('Has errors')
    ).toHaveCount(0);
    await variablesTab.click();
    await expect(toggle).toHaveAttribute('aria-expanded', 'true');
    await expect(
      page.getByText('Reserved name — choose another.')
    ).toBeVisible();
    expect(edit.bodies).toEqual([]);
  });

  test('only the section card scrolls, not the page or tab list', async ({
    page
  }) => {
    await page.setViewportSize({ height: 600, width: 1280 });
    await gotoReady(page, '/eggs/egg-paper/general');
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

  test('variables load collapsed and toggle open', async ({ page }) => {
    await gotoReady(page, '/eggs/egg-paper/general');
    await openTab(page, 'Variables');
    const toggle = page.getByRole('button', { name: 'Toggle variable' });
    const header = page.locator('[data-slot="card-title"]', { has: toggle });
    await expect(header).toContainText('SERVER_JARFILE');
    await expect(toggle).toHaveAttribute('aria-expanded', 'false');
    await expect(page.getByLabel('Env Variable')).toBeHidden();
    await toggle.click();
    await expect(toggle).toHaveAttribute('aria-expanded', 'true');
    await expect(page.getByLabel('Env Variable')).toBeVisible();
    await toggle.click();
    await expect(page.getByLabel('Env Variable')).toBeHidden();
    await page.getByRole('button', { name: 'Add variable' }).click();
    await expect(toggle.nth(1)).toHaveAttribute('aria-expanded', 'true');
    await expect(header.nth(1)).toContainText('New variable');
    await page.getByLabel('Env Variable').nth(1).fill('NEW_ENV');
    await expect(header.nth(1)).toContainText('NEW_ENV');
    await expectNoHorizontalOverflow(page);
  });

  test('shows one section per tab', async ({ page }) => {
    await gotoReady(page, '/eggs/egg-paper/general');
    await expect(page.getByLabel('Egg Name')).toBeVisible();
    await expect(page.getByText('Startup variables (1)')).toBeHidden();
    await openTab(page, 'Variables');
    await expect(page.getByText('Startup variables (1)')).toBeVisible();
    await expect(page.getByLabel('Egg Name')).toBeHidden();
    await expectNoHorizontalOverflow(page);
  });

  test('export menu items stay on one line', async ({ page }) => {
    await gotoReady(page, '/eggs/egg-paper/general');
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
      await gotoReady(page, '/eggs/egg-paper/general');
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
      await expect(page.getByText('Export downloaded')).toBeVisible();
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

  test('check for updates without an update URL shows a toast', async ({
    page
  }) => {
    await gotoReady(page, '/eggs/egg-paper/general');
    await expect(page.getByLabel('Egg Name')).toHaveValue('Paper');
    await page.getByRole('button', { name: 'Check for updates' }).click();
    await expect(page.getByText('No update URL')).toBeVisible();
  });

  test('checks the egg for updates and applies them', async ({
    network,
    page
  }) => {
    const withUrl = {
      ...eggInfo,
      meta: { update_url: 'https://eggs.example.com/paper.json' }
    };
    network.use(
      gen.handleEggInfo(() => HttpResponse.json(withUrl) as never),
      sourceHandler({ 'egg-paper': { ...changedSource, meta: withUrl.meta } })
    );
    await gotoReady(page, '/eggs/egg-paper/general');
    await expect(page.getByLabel('Egg Name')).toHaveValue('Paper');
    await page.getByRole('button', { name: 'Check for updates' }).click();
    await applyUpdate(page);
    await expect(page.locator('textarea[name="description"]')).toHaveValue(
      'Paper, now faster'
    );
    await expect(page.getByText('Unsaved changes')).toBeHidden();
  });

  test('shows check failures in a toast with retry', async ({
    network,
    page
  }) => {
    network.use(
      gen.handleEggInfo(
        () =>
          HttpResponse.json({
            ...eggInfo,
            meta: { update_url: 'https://eggs.example.com/paper.json' }
          }) as never
      ),
      sourceHandler({})
    );
    await gotoReady(page, '/eggs/egg-paper/general');
    await expect(page.getByLabel('Egg Name')).toHaveValue('Paper');
    await page.getByRole('button', { name: 'Check for updates' }).click();
    await expect(page.getByText("Couldn't check for updates")).toBeVisible();
    await expect(
      page.getByText('Could not reach the update URL')
    ).toBeVisible();
    await expect(page.getByRole('button', { name: 'Retry' })).toBeVisible();
  });

  test('deletes the egg and returns to the list', async ({ network, page }) => {
    const del = recorder();
    network.use(
      gen.handleDeleteEgg(async ({ request }) => {
        await del.record(request);
        return new HttpResponse(null, { status: 200 }) as never;
      })
    );
    await gotoReady(page, '/eggs/egg-paper/general');
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

  test('detail is read-only but can still export', async ({ page }) => {
    await gotoReady(page, '/eggs/egg-paper/general');
    await expect(page.getByLabel('Egg Name')).toHaveValue('Paper');
    await expect(page.getByLabel('Egg Name')).toBeDisabled();
    await expect(
      page.getByRole('switch', { name: 'EULA prompt' })
    ).toBeDisabled();
    await expect(
      page.getByRole('button', { name: 'Save Changes' })
    ).toBeDisabled();
    await openTab(page, 'Variables');
    await page.getByRole('button', { name: 'Toggle variable' }).click();
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

test.describe('egg detail: the URL names the open tab', () => {
  test.beforeEach(async ({ context }) => setupSession(context));

  for (const [path, name, url] of [
    ['', 'General', /\/eggs\/egg-paper\/general$/],
    ['/general', 'General', /\/eggs\/egg-paper\/general$/],
    ['/variables', 'Variables', /\/eggs\/egg-paper\/variables$/],
    ['/install', 'Install script', /\/eggs\/egg-paper\/install$/],
    ['/runtime', 'Runtime', /\/eggs\/egg-paper\/runtime$/],
    ['/config', 'Config', /\/eggs\/egg-paper\/config$/]
  ] as const) {
    test(`/eggs/egg-paper${path} opens ${name}`, async ({ page }) => {
      await gotoReady(page, `/eggs/egg-paper${path}`);

      await expect(page).toHaveURL(url);
      await expect(page.getByRole('tab', { name })).toHaveAttribute(
        'aria-selected',
        'true'
      );
      await expect(page.getByRole('tab', { selected: true })).toHaveCount(1);
    });
  }

  for (const path of ['bogus', 'settings', 'General', 'install-script']) {
    test(`/eggs/egg-paper/${path} is a 404`, async ({ page }) => {
      const res = await page.goto(`/eggs/egg-paper/${path}`);

      expect(res?.status()).toBe(404);
      await expect(page.getByRole('tab')).toHaveCount(0);
    });
  }

  test('clicking tabs pushes history without refetching the egg', async ({
    page
  }) => {
    await gotoReady(page, '/eggs/egg-paper/general');
    await expect(page.getByLabel('Egg Name')).toHaveValue('Paper');
    const fetched: string[] = [];
    page.on('request', (req) => {
      if (new URL(req.url()).pathname.startsWith('/api/eggs')) {
        fetched.push(req.url());
      }
    });

    await openTab(page, 'Variables');
    await expect(page).toHaveURL(/\/eggs\/egg-paper\/variables$/);
    await openTab(page, 'Runtime');
    await expect(page).toHaveURL(/\/eggs\/egg-paper\/runtime$/);
    await expect(page.getByRole('tab', { name: 'Runtime' })).toHaveAttribute(
      'aria-selected',
      'true'
    );

    await page.goBack();
    await expect(page).toHaveURL(/\/eggs\/egg-paper\/variables$/);
    await expect(page.getByRole('tab', { name: 'Variables' })).toHaveAttribute(
      'aria-selected',
      'true'
    );
    await page.goBack();
    await expect(page).toHaveURL(/\/eggs\/egg-paper\/general$/);
    await expect(page.getByRole('tab', { name: 'General' })).toHaveAttribute(
      'aria-selected',
      'true'
    );
    await page.goForward();
    await expect(page.getByRole('tab', { name: 'Variables' })).toHaveAttribute(
      'aria-selected',
      'true'
    );
    await page.waitForLoadState('networkidle');
    expect(fetched).toEqual([]);
  });

  test('unsaved edits survive tab switches and back navigation', async ({
    page
  }) => {
    await gotoReady(page, '/eggs/egg-paper/general');
    const name = page.getByLabel('Egg Name');
    await expect(name).toHaveValue('Paper');
    await name.fill('Draft Paper');
    await expect(page.getByText('Unsaved changes')).toBeVisible();

    await openTab(page, 'Install script');
    await expect(page).toHaveURL(/\/eggs\/egg-paper\/install$/);
    await openTab(page, 'Config');
    await page.goBack();
    await page.goBack();
    await expect(page).toHaveURL(/\/eggs\/egg-paper\/general$/);

    await expect(name).toHaveValue('Draft Paper');
    await expect(page.getByText('Unsaved changes')).toBeVisible();
  });
});

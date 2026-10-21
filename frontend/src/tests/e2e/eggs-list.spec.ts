import { expect } from '@playwright/test';
import { HttpResponse } from 'msw';
import * as gen from '$lib/client/msw.gen';
import { test } from '$test_helpers/e2e-fixture';
import { setupSession } from '$test_helpers/session';
import { expectNoHorizontalOverflow, gotoReady } from '$test_helpers/layout';
import {
  applyUpdate,
  changedSource,
  ptdlFile,
  recorder,
  sourceHandler
} from '$test_helpers/eggs-e2e';

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
    await expect(
      page.getByRole('link', { exact: true, name: 'Create' })
    ).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Import JSON' })
    ).toBeVisible();
    await expect(page.getByText('2 eggs')).toBeVisible();
    await expectNoHorizontalOverflow(page);
  });

  test('links the egg name to its detail page', async ({ page }) => {
    await gotoReady(page, '/eggs');
    await page.getByRole('link', { exact: true, name: 'Paper' }).click();
    await expect(page).toHaveURL(/\/eggs\/egg-paper\/general$/);
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
    await expect(
      page.getByRole('link', { exact: true, name: 'Create' })
    ).toHaveCount(1);
    await expect(page.getByRole('button', { name: 'Import JSON' })).toHaveCount(
      1
    );
    await expectNoHorizontalOverflow(page);
    await page.getByRole('link', { exact: true, name: 'Create' }).click();
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

  test('checks all eggs and applies a reviewed update', async ({
    network,
    page
  }) => {
    const edit = recorder();
    network.use(
      sourceHandler({ 'egg-paper': changedSource }),
      gen.handleEditEgg(async ({ request }) => {
        await edit.record(request);
        return new HttpResponse(null, { status: 200 }) as never;
      })
    );
    await gotoReady(page, '/eggs');
    await page.getByRole('button', { name: 'Check for updates' }).click();
    await expect(page.getByText('1 update available')).toBeVisible();
    await expect(page.getByText("Couldn't check 1 egg")).toBeVisible();
    await expect(page.getByText('Check failed')).toBeVisible();
    await page.getByRole('button', { name: 'Review update for Paper' }).click();
    await applyUpdate(page);
    expect(edit.bodies).toHaveLength(1);
    expect(edit.bodies[0]).toMatchObject({
      description: 'Paper, now faster',
      scripts: { installation: { script: 'echo install\necho updated' } },
      uuid: 'egg-paper'
    });
  });

  test('reports up-to-date eggs in a toast', async ({ page }) => {
    await gotoReady(page, '/eggs');
    await page.getByRole('button', { name: 'Check for updates' }).click();
    await expect(page.getByText('All eggs are up to date')).toBeVisible();
    await expect(page.getByText('Update available')).toHaveCount(0);
  });

  test('shows the in-use error when delete conflicts', async ({
    network,
    page
  }) => {
    network.use(
      gen.handleDeleteEgg(
        () => new HttpResponse(null, { status: 409 }) as never
      )
    );
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
    await expect(
      page.getByText(
        'Egg is still used by servers. Delete them or change their egg first.'
      )
    ).toBeVisible();
    await expect(page.getByText(/deleted successfully/)).toHaveCount(0);
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

test.describe('eggs without egg:edit', () => {
  test.beforeEach(async ({ context }) => setupSession(context, 'readonly'));

  test('list disables create, edit and delete', async ({ page }) => {
    await gotoReady(page, '/eggs');
    await expect(
      page.getByRole('cell', { exact: true, name: 'Paper' })
    ).toBeVisible();
    await expect(
      page.getByRole('link', { exact: true, name: 'Create' })
    ).toHaveAttribute('aria-disabled', 'true');
    await expect(
      page.getByRole('row', { name: /Paper/ }).getByRole('button').last()
    ).toBeDisabled();
  });
});

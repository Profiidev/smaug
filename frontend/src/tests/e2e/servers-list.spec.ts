import { expect } from '@playwright/test';
import { HttpResponse } from 'msw';
import * as gen from '$lib/client/msw.gen';
import { test } from '$test_helpers/e2e-fixture';
import { setupSession } from '$test_helpers/session';
import { expectNoHorizontalOverflow, gotoReady } from '$test_helpers/layout';
import { choose, ok, row } from '$test_helpers/servers-e2e';

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
      page.getByRole('link', { exact: true, name: 'Create' })
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

    await choose(page, filter, 'Suspended');
    await expect(row(page, 'Banned Realm')).toBeVisible();
    await expect(row(page, 'Survival SMP')).toBeHidden();
    await expect(page.getByText('1 server', { exact: true })).toBeVisible();

    await choose(page, filter, 'All statuses');
    await expect(page.getByText('5 servers')).toBeVisible();
  });

  test('shows the empty state', async ({ context, page }) => {
    await setupSession(context, 'empty');
    await gotoReady(page, '/servers');

    await expect(page.getByText('No servers yet')).toBeVisible();
    // The empty state carries the only create action and no filter.
    await expect(
      page.getByRole('link', { exact: true, name: 'Create' })
    ).toHaveCount(1);
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

test.describe('servers without server:edit', () => {
  test.beforeEach(async ({ context }) => setupSession(context, 'readonly'));

  test('list disables create and delete but not edit', async ({ page }) => {
    await gotoReady(page, '/servers');

    await expect(row(page, 'Survival SMP')).toBeVisible();
    await expect(
      page.getByRole('link', { exact: true, name: 'Create' })
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

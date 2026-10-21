import { type Page, expect } from '@playwright/test';
import { HttpResponse } from 'msw';
import * as gen from '$lib/client/msw.gen';
import { test } from '$test_helpers/e2e-fixture';
import { setupSession } from '$test_helpers/session';
import { expectNoHorizontalOverflow, gotoReady } from '$test_helpers/layout';

const ok = () => new HttpResponse(null, { status: 200 }) as never;

const openDelete = async (page: Page) => {
  await page
    .getByRole('row', { name: /Node Two/ })
    .getByRole('button')
    .last()
    .click();
  return page.getByRole('dialog');
};

test.describe('nodes list', () => {
  test.beforeEach(async ({ context }) => setupSession(context));

  test('lists the nodes and fits the viewport', async ({ page }) => {
    await gotoReady(page, '/nodes');

    await expect(page.getByRole('heading', { name: 'Nodes' })).toBeVisible();
    await expect(page.getByRole('link', { name: 'Node One' })).toHaveAttribute(
      'href',
      '/nodes/node-1/setup'
    );
    await expect(page.getByText('node1.example.com:443')).toBeVisible();
    await expect(page.getByText('10.0.0.2:8000')).toBeVisible();
    await expect(page.getByText('2 nodes')).toBeVisible();
    await expect(page.getByRole('link', { name: 'Create' })).toBeVisible();
    await expectNoHorizontalOverflow(page);
  });

  test('searches the nodes', async ({ page }) => {
    await gotoReady(page, '/nodes');

    await page.getByPlaceholder('Search...').fill('10.0.0');
    await expect(page.getByRole('link', { name: 'Node Two' })).toBeVisible();
    await expect(page.getByRole('link', { name: 'Node One' })).toBeHidden();
  });

  test('opens a node from its name', async ({ page }) => {
    await gotoReady(page, '/nodes');

    await page.getByRole('link', { name: 'Node One' }).click();
    await expect(page).toHaveURL(/\/nodes\/node-1\/setup$/);
    await expect(page.getByRole('heading', { name: /Node:/ })).toContainText(
      'Node One'
    );
  });

  test('shows an empty state with no nodes', async ({ context, page }) => {
    await setupSession(context, 'empty');
    await gotoReady(page, '/nodes');

    await expect(page.getByText('No nodes yet')).toBeVisible();
    await expect(page.getByRole('link', { name: 'Create' })).toHaveCount(1);
    await expect(page.getByRole('table')).toBeHidden();
    await expectNoHorizontalOverflow(page);
  });

  test('deletes a node from the list', async ({ network, page }) => {
    const bodies: unknown[] = [];
    network.use(
      gen.handleDeleteNode(async ({ request }) => {
        bodies.push(await request.json());
        return ok();
      })
    );
    await gotoReady(page, '/nodes');

    const dialog = await openDelete(page);
    await expect(dialog.getByText('Delete node Node Two?')).toBeVisible();
    await dialog.getByRole('button', { name: 'Delete' }).click();

    await expect(
      page.getByText('Node Node Two deleted successfully')
    ).toBeVisible();
    await expect(dialog).toBeHidden();
    expect(bodies).toEqual([{ uuid: 'node-2' }]);
  });

  test('shows the in-use error when delete conflicts', async ({
    network,
    page
  }) => {
    network.use(
      gen.handleDeleteNode(
        () => new HttpResponse(null, { status: 409 }) as never
      )
    );
    await gotoReady(page, '/nodes');

    const dialog = await openDelete(page);
    await dialog.getByRole('button', { name: 'Delete' }).click();

    await expect(
      page.getByText('Node still hosts servers. Delete or transfer them first.')
    ).toBeVisible();
    expect(page.url()).toMatch(/\/nodes$/);
  });

  test('shows an error when delete fails', async ({ network, page }) => {
    network.use(
      gen.handleDeleteNode(
        () => new HttpResponse(null, { status: 500 }) as never
      )
    );
    await gotoReady(page, '/nodes');

    const dialog = await openDelete(page);
    await dialog.getByRole('button', { name: 'Delete' }).click();

    await expect(page.getByText('Failed to delete node')).toBeVisible();
  });
});

test.describe('nodes list without node:edit', () => {
  test.beforeEach(async ({ context }) => setupSession(context, 'readonly'));

  test('disables create, edit and delete', async ({ page }) => {
    await gotoReady(page, '/nodes');
    await expect(page.getByRole('link', { name: 'Node One' })).toBeVisible();

    await expect(page.getByRole('link', { name: 'Create' })).toHaveAttribute(
      'aria-disabled',
      'true'
    );
    const row = page.getByRole('row', { name: /Node One/ });
    await expect(row.getByRole('link').last()).toHaveAttribute(
      'aria-disabled',
      'true'
    );
    await expect(row.getByRole('button').last()).toBeDisabled();
  });
});

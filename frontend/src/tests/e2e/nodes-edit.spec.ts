import { expect } from '@playwright/test';
import { HttpResponse } from 'msw';
import * as gen from '$lib/client/msw.gen';
import { test } from '$test_helpers/e2e-fixture';
import { setupSession } from '$test_helpers/session';
import { expectNoHorizontalOverflow, gotoReady } from '$test_helpers/layout';
import { openTab, save, tab } from '$test_helpers/servers-e2e';

const ok = () => new HttpResponse(null, { status: 200 }) as never;

test.describe('node edit', () => {
  test.beforeEach(async ({ context }) => setupSession(context));

  test('opens the setup tab and fits the viewport', async ({ page }) => {
    await gotoReady(page, '/nodes/node-1');

    await expect(page.getByRole('heading', { name: /Node:/ })).toContainText(
      'Node One'
    );
    await expect(tab(page, 'Setup')).toHaveAttribute('aria-selected', 'true');
    await expect(page.getByLabel('Node Auth Token')).toHaveValue(
      'Can not be viewed.'
    );
    await expect(page.locator('pre.shiki')).toContainText('- TOKEN=<TOKEN>');
    await expect(
      page.getByRole('button', { name: 'Save Changes' })
    ).toBeHidden();
    await expectNoHorizontalOverflow(page);

    for (const name of ['General', 'Resources']) {
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

  test.describe('old sub-routes redirect to tabs', () => {
    for (const [path, name, url] of [
      ['setup', 'Setup', /\/nodes\/node-1\?tab=setup$/],
      ['settings', 'General', /\/nodes\/node-1\?tab=general$/]
    ] as const) {
      test(`/${path}`, async ({ page }) => {
        await gotoReady(page, `/nodes/node-1/${path}`);

        await expect(page).toHaveURL(url);
        await expect(tab(page, name)).toHaveAttribute('aria-selected', 'true');
      });
    }
  });

  test('regenerates the node token', async ({ page }) => {
    await gotoReady(page, '/nodes/node-1');

    await page.getByRole('button', { name: 'Regenerate' }).click();
    const dialog = page.getByRole('dialog');
    await expect(
      dialog.getByText('Regenerate token for Node One?')
    ).toBeVisible();
    await dialog.getByRole('button', { name: 'Regenerate' }).click();

    await expect(
      page.getByText('Token for node Node One regenerated successfully')
    ).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'node-regenerated-token' })
    ).toBeVisible();
    await expect(page.getByText('(Can not be viewed again!)')).toBeVisible();
    await expect(page.locator('pre.shiki')).toContainText(
      '- TOKEN=node-regenerated-token'
    );
  });

  test('shows an error when regenerate fails', async ({ network, page }) => {
    network.use(
      gen.handleNodeTokenRegenerate(
        () => new HttpResponse(null, { status: 500 }) as never
      )
    );
    await gotoReady(page, '/nodes/node-1');

    await page.getByRole('button', { name: 'Regenerate' }).click();
    await page
      .getByRole('dialog')
      .getByRole('button', { name: 'Regenerate' })
      .click();

    await expect(page.getByText('Failed to regenerate token')).toBeVisible();
  });

  test('updates the node settings', async ({ network, page }) => {
    const bodies: unknown[] = [];
    network.use(
      gen.handleUpdateNode(async ({ params, request }) => {
        bodies.push({ body: await request.json(), uuid: params.uuid });
        return ok();
      })
    );
    await gotoReady(page, '/nodes/node-1?tab=general');

    const name = page.getByPlaceholder('Enter name');
    await expect(name).toHaveValue('Node One');
    await expect(
      page.getByPlaceholder('Enter address (e.g., IP or domain)')
    ).toHaveValue('node1.example.com:443');
    await name.fill('Node Renamed');
    await expect(page.getByText('Unsaved changes')).toBeVisible();
    await openTab(page, 'Resources');
    await page.getByRole('switch', { name: 'Unlimited Storage' }).click();
    await page.getByPlaceholder('Enter amount of storage').fill('50');
    await save(page);

    await expect(
      page.getByText('Node Node Renamed updated successfully')
    ).toBeVisible();
    await expect(page.getByRole('heading', { name: /Node:/ })).toContainText(
      'Node Renamed'
    );
    await expect(page.getByText('Unsaved changes')).toBeHidden();
    expect(bodies).toEqual([
      {
        body: {
          address: 'node1.example.com:443',
          // The unit select pre-fills MiB for the 100 GiB default; 50 MiB in MB.
          disk_limit_mb: 52.4288,
          name: 'Node Renamed',
          secure: true
        },
        uuid: 'node-1'
      }
    ]);
  });

  for (const [status, message] of [
    [409, 'This node name is already in use'],
    [400, 'Invalid node address']
  ] as const) {
    test(`shows the ${status} error on the General tab`, async ({
      network,
      page
    }) => {
      network.use(
        gen.handleUpdateNode(() => new HttpResponse(null, { status }) as never)
      );
      await gotoReady(page, '/nodes/node-1?tab=resources');
      await openTab(page, 'General');
      await expect(page.getByPlaceholder('Enter name')).toHaveValue('Node One');
      await openTab(page, 'Resources');

      await save(page);

      await expect(tab(page, 'General')).toHaveAttribute(
        'aria-selected',
        'true'
      );
      await expect(page.getByText(message).first()).toBeVisible();
      await expect(tab(page, 'General').getByLabel('Has errors')).toBeVisible();
    });
  }

  test('shows retry when saving fails', async ({ network, page }) => {
    network.use(
      gen.handleUpdateNode(
        () => new HttpResponse(null, { status: 500 }) as never
      )
    );
    await gotoReady(page, '/nodes/node-1?tab=general');
    await expect(page.getByPlaceholder('Enter name')).toHaveValue('Node One');

    await save(page);

    await expect(page.getByText('Failed to update node')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Retry' })).toBeVisible();
  });

  test('deletes the node and returns to the list', async ({
    network,
    page
  }) => {
    const bodies: unknown[] = [];
    network.use(
      gen.handleDeleteNode(async ({ request }) => {
        bodies.push(await request.json());
        return ok();
      })
    );
    await gotoReady(page, '/nodes/node-1');
    await expect(page.getByRole('heading', { name: /Node:/ })).toContainText(
      'Node One'
    );

    await page.getByRole('button', { name: 'Delete' }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog.getByText('Delete node Node One?')).toBeVisible();
    await dialog.getByRole('button', { name: 'Delete' }).click();

    await expect(page).toHaveURL(/\/nodes$/);
    await expect(
      page.getByText('Node Node One deleted successfully')
    ).toBeVisible();
    expect(bodies).toEqual([{ uuid: 'node-1' }]);
  });

  test('redirects an unknown node to the list with a toast', async ({
    page
  }) => {
    await gotoReady(page, '/nodes/node-missing');

    await expect(page).toHaveURL(/\/nodes$/);
    await expect(page.getByText('Node not found')).toBeVisible();
  });

  test('redirects other load failures with a generic toast', async ({
    network,
    page
  }) => {
    network.use(
      gen.handleNodeInfo(() => new HttpResponse(null, { status: 500 }) as never)
    );
    await gotoReady(page, '/nodes/node-1');

    await expect(page).toHaveURL(/\/nodes$/);
    await expect(page.getByText('Failed to load node')).toBeVisible();
  });
});

test.describe('node edit without node:edit', () => {
  test.beforeEach(async ({ context }) => setupSession(context, 'readonly'));

  test('fields, save, delete and regenerate are disabled', async ({ page }) => {
    await gotoReady(page, '/nodes/node-1?tab=general');
    await expect(page.getByPlaceholder('Enter name')).toHaveValue('Node One');

    await expect(page.getByPlaceholder('Enter name')).toBeDisabled();
    await expect(
      page.getByRole('button', { name: 'Save Changes' })
    ).toBeDisabled();
    await expect(page.getByRole('button', { name: 'Delete' })).toBeDisabled();
    await openTab(page, 'Resources');
    await expect(
      page.getByRole('switch', { name: 'Unlimited CPU' })
    ).toBeDisabled();
    await openTab(page, 'Setup');
    await expect(
      page.getByRole('button', { name: 'Regenerate' })
    ).toBeDisabled();
    await expectNoHorizontalOverflow(page);
  });
});

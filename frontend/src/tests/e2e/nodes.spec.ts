import { expect } from '@playwright/test';
import { HttpResponse } from 'msw';
import * as gen from '$lib/client/msw.gen';
import { test } from '$test_helpers/e2e-fixture';
import { setupSession } from '$test_helpers/session';
import { expectNoHorizontalOverflow, gotoReady } from '$test_helpers/layout';
import { nodeInfo } from '../mocks/e2e/data';

const ok = () => new HttpResponse(null, { status: 200 }) as never;

test.describe('nodes list', () => {
  test.beforeEach(async ({ context }) => setupSession(context));

  test('lists the nodes and fits the viewport', async ({ page }) => {
    await gotoReady(page, '/nodes');

    await expect(page.getByRole('heading', { name: 'Nodes' })).toBeVisible();
    await expect(page.getByRole('cell', { name: 'Node One' })).toBeVisible();
    await expect(
      page.getByRole('cell', { name: 'node1.example.com:443' })
    ).toBeVisible();
    await expect(
      page.getByRole('cell', { name: '10.0.0.2:8000' })
    ).toBeVisible();
    await expect(page.getByRole('link', { name: 'Create' })).toBeVisible();
    await expect(
      page.getByRole('row', { name: /Node One/ }).getByRole('link')
    ).toHaveAttribute('href', '/nodes/node-1/setup');
    await expectNoHorizontalOverflow(page);
  });

  test('shows an empty state with no nodes', async ({ context, page }) => {
    await setupSession(context, 'empty');
    await gotoReady(page, '/nodes');

    await expect(page.getByText('No results.')).toBeVisible();
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

    await page
      .getByRole('row', { name: /Node Two/ })
      .getByRole('button')
      .last()
      .click();
    const dialog = page.getByRole('dialog');
    await expect(
      dialog.getByText('Do you really want to delete the node Node Two?')
    ).toBeVisible();
    await dialog.getByRole('button', { name: 'Delete' }).click();

    await expect(
      page.getByText('Node Node Two deleted successfully')
    ).toBeVisible();
    await expect(dialog).toBeHidden();
    expect(bodies).toEqual([{ uuid: 'node-2' }]);
  });

  test('shows an error when delete fails', async ({ network, page }) => {
    network.use(
      gen.handleDeleteNode(
        () => new HttpResponse(null, { status: 500 }) as never
      )
    );
    await gotoReady(page, '/nodes');

    await page
      .getByRole('row', { name: /Node Two/ })
      .getByRole('button')
      .last()
      .click();
    const dialog = page.getByRole('dialog');
    await dialog.getByRole('button', { name: 'Delete' }).click();

    await expect(page.getByText('Failed to delete node')).toBeVisible();
    expect(page.url()).toMatch(/\/nodes$/);
  });
});

test.describe('node create', () => {
  test.beforeEach(async ({ context }) => setupSession(context));

  test('creates a node and shows its setup commands', async ({
    network,
    page
  }) => {
    const bodies: unknown[] = [];
    network.use(
      gen.handleCreateNode(async ({ request }) => {
        bodies.push(await request.json());
        return HttpResponse.json({
          token: 'fresh-token',
          uuid: 'node-new'
        }) as never;
      }),
      gen.handleNodeInfo(
        () =>
          HttpResponse.json({
            ...nodeInfo,
            connected: false,
            id: 'node-new',
            name: 'Fresh Node'
          }) as never
      )
    );
    await gotoReady(page, '/nodes/create');

    // General Settings
    await expect(page.getByText('General Settings')).toBeVisible();
    await page.getByPlaceholder('Enter name').fill('Fresh Node');
    await page
      .getByPlaceholder('Enter address (e.g., IP or domain)')
      .fill('fresh.example.com:8443');
    await expectNoHorizontalOverflow(page);
    await page.getByRole('button', { name: 'Next' }).click();

    // Advanced Settings: limit CPU and memory, keep storage unlimited.
    await expect(page.getByText('Advanced Settings')).toBeVisible();
    await page.getByRole('switch', { name: 'Unlimited CPU' }).click();
    await page.getByPlaceholder('Enter CPU limit').fill('1500');
    await page.getByRole('switch', { name: 'Unlimited Memory' }).click();
    await page.getByPlaceholder('Enter amount of memory').fill('2000');
    await expectNoHorizontalOverflow(page);
    await page.getByRole('button', { name: 'Create' }).click();

    await expect(page).toHaveURL(/\/nodes\/node-new\/setup$/);
    await expect(page.getByText('Node created successfully.')).toBeVisible();
    expect(bodies).toEqual([
      {
        address: 'fresh.example.com:8443',
        cpu_limit: 1500,
        // 2000 MiB in MB.
        memory_limit_mb: 2097.152,
        name: 'Fresh Node',
        secure: true
      }
    ]);

    // Setup page: token plus compose (default) and run commands.
    await expect(
      page.getByRole('heading', { name: 'Node Setup' })
    ).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'fresh-token' })
    ).toBeVisible();
    const code = page.locator('pre.shiki');
    await expect(code).toContainText('smaug-wings:');
    await expect(code).toContainText('- TOKEN=fresh-token');
    await expect(code).toContainText('/var/run/docker.sock');

    await page.getByRole('button', { name: 'Docker Compose' }).click();
    await page.getByRole('option', { name: 'Docker Run' }).click();
    await expect(code).toContainText('docker run -d');
    await expect(code).toContainText('-e TOKEN=fresh-token');
    await expectNoHorizontalOverflow(page);

    // The token is only shown once.
    await page.reload();
    await expect(page.locator('input.text-destructive')).toHaveValue(
      'Can not be viewed.'
    );
    await expect(code).toContainText('- TOKEN=<TOKEN>');
  });

  test('blocks an empty first step', async ({ network, page }) => {
    let called = false;
    network.use(
      gen.handleCreateNode(() => {
        called = true;
        return HttpResponse.json({ uuid: 'node-new' }) as never;
      })
    );
    await gotoReady(page, '/nodes/create');

    await page.getByRole('button', { name: 'Next' }).click();

    await expect(page.getByText('Name is required')).toBeVisible();
    await expect(page.getByText('Address is required')).toBeVisible();
    await expect(page).toHaveURL(/\/nodes\/create$/);
    expect(called).toBe(false);
  });

  test('shows the duplicate-name error from the server', async ({
    network,
    page
  }) => {
    network.use(
      gen.handleCreateNode(
        () => new HttpResponse(null, { status: 409 }) as never
      )
    );
    await gotoReady(page, '/nodes/create');

    await page.getByPlaceholder('Enter name').fill('Node One');
    await page
      .getByPlaceholder('Enter address (e.g., IP or domain)')
      .fill('node1.example.com');
    await page.getByRole('button', { name: 'Next' }).click();
    await page.getByRole('button', { name: 'Create' }).click();

    await expect(
      page.getByText('A node with this name already exists.')
    ).toBeVisible();
    await expect(page).toHaveURL(/\/nodes\/create$/);
  });
});

test.describe('node detail', () => {
  test.beforeEach(async ({ context }) => setupSession(context));

  test('redirects /nodes/[uuid] to its setup page', async ({ page }) => {
    await gotoReady(page, '/nodes/node-1');

    await expect(page).toHaveURL(/\/nodes\/node-1\/setup$/);
    await expect(page.getByRole('heading', { name: /Node:/ })).toContainText(
      'Node One'
    );
    await expect(page.locator('input.text-destructive')).toHaveValue(
      'Can not be viewed.'
    );
  });

  test('regenerates the node token', async ({ page }) => {
    await gotoReady(page, '/nodes/node-1/setup');

    await page.getByRole('button', { name: 'Regenerate' }).click();
    await page
      .getByRole('dialog')
      .getByRole('button', { name: 'Regenerate' })
      .click();

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

  test('updates the node settings', async ({ network, page }) => {
    const bodies: unknown[] = [];
    network.use(
      gen.handleUpdateNode(async ({ params, request }) => {
        bodies.push({ body: await request.json(), uuid: params.uuid });
        return ok();
      })
    );
    await gotoReady(page, '/nodes/node-1/settings');

    const name = page.getByPlaceholder('Enter name');
    await expect(name).toHaveValue('Node One');
    await expect(
      page.getByPlaceholder('Enter address (e.g., IP or domain)')
    ).toHaveValue('node1.example.com:443');
    await name.fill('Node Renamed');
    await page.getByRole('switch', { name: 'Unlimited Storage' }).click();
    await page.getByPlaceholder('Enter amount of storage').fill('50');
    await page.getByRole('button', { name: 'Save Changes' }).click();

    await expect(
      page.getByText('Node Node Renamed updated successfully')
    ).toBeVisible();
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

  test('shows the duplicate-name error on save', async ({ network, page }) => {
    network.use(
      gen.handleUpdateNode(
        () => new HttpResponse(null, { status: 409 }) as never
      )
    );
    await gotoReady(page, '/nodes/node-1/settings');
    await expect(page.getByPlaceholder('Enter name')).toHaveValue('Node One');

    await page.getByRole('button', { name: 'Save Changes' }).click();

    await expect(
      page.getByText('This node name is already in use')
    ).toBeVisible();
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
    await gotoReady(page, '/nodes/node-1/setup');
    await expect(page.getByRole('heading', { name: /Node:/ })).toContainText(
      'Node One'
    );

    await page.getByRole('button', { name: 'Delete' }).click();
    await page
      .getByRole('dialog')
      .getByRole('button', { name: 'Delete' })
      .click();

    await expect(page).toHaveURL(/\/nodes$/);
    await expect(
      page.getByText('Node Node One deleted successfully')
    ).toBeVisible();
    expect(bodies).toEqual([{ uuid: 'node-1' }]);
  });

  test('redirects an unknown node to the list with a toast', async ({
    page
  }) => {
    await gotoReady(page, '/nodes/node-missing/setup');

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
    await gotoReady(page, '/nodes/node-1/setup');

    await expect(page).toHaveURL(/\/nodes$/);
    await expect(page.getByText('Failed to load node')).toBeVisible();
  });
});

test.describe('nodes without node:edit', () => {
  test.beforeEach(async ({ context }) => setupSession(context, 'readonly'));

  test('list disables create, edit and delete', async ({ page }) => {
    await gotoReady(page, '/nodes');
    await expect(page.getByRole('cell', { name: 'Node One' })).toBeVisible();

    await expect(page.getByRole('link', { name: 'Create' })).toHaveAttribute(
      'aria-disabled',
      'true'
    );
    const row = page.getByRole('row', { name: /Node One/ });
    await expect(row.getByRole('link')).toHaveAttribute(
      'aria-disabled',
      'true'
    );
    await expect(row.getByRole('button').last()).toBeDisabled();
  });

  test('settings are read-only and delete is disabled', async ({ page }) => {
    await gotoReady(page, '/nodes/node-1/settings');
    await expect(page.getByPlaceholder('Enter name')).toHaveValue('Node One');

    await expect(page.getByPlaceholder('Enter name')).toHaveAttribute(
      'readonly'
    );
    await expect(
      page.getByRole('switch', { name: 'Unlimited CPU' })
    ).toBeDisabled();
    await expect(page.getByRole('button', { name: 'Delete' })).toBeDisabled();
    await expect(
      page.getByRole('button', { name: 'Save Changes' })
    ).toBeDisabled();
    await expectNoHorizontalOverflow(page);
  });
});

import { expect } from '@playwright/test';
import { HttpResponse } from 'msw';
import * as gen from '$lib/client/msw.gen';
import { test } from '$test_helpers/e2e-fixture';
import { setupSession } from '$test_helpers/session';
import { expectNoHorizontalOverflow, gotoReady } from '$test_helpers/layout';
import { tab } from '$test_helpers/servers-e2e';
import { nodeInfo } from '../mocks/e2e/data';

test.describe('node create', () => {
  test.beforeEach(async ({ context }) => setupSession(context));

  test('creates a node and shows its token on the setup tab', async ({
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

    await expect(
      page.getByRole('heading', { name: 'Create node' })
    ).toBeVisible();
    await expect(page.getByText('General', { exact: true })).toBeVisible();
    await page.getByPlaceholder('Enter name').fill('Fresh Node');
    await page
      .getByPlaceholder('Enter address (e.g., IP or domain)')
      .fill('fresh.example.com:8443');
    await expectNoHorizontalOverflow(page);
    await page.getByRole('button', { name: 'Next' }).click();

    // Resources: limit CPU and memory, keep storage unlimited.
    await expect(page.getByText('Resources', { exact: true })).toBeVisible();
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

    // Setup tab: token plus compose (default) and run commands.
    await expect(tab(page, 'Setup')).toHaveAttribute('aria-selected', 'true');
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
    await expect(page.getByLabel('Node Auth Token')).toHaveValue(
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

  test('warns about plain HTTP', async ({ page }) => {
    await gotoReady(page, '/nodes/create');

    await page
      .getByRole('switch', { name: 'Use Secure Connection (HTTPS)' })
      .click();
    await expect(
      page.getByText('Only use HTTP on private networks!')
    ).toBeVisible();
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

  test('goes back to the list', async ({ page }) => {
    await gotoReady(page, '/nodes/create');

    await page.getByRole('link', { name: 'Back to nodes' }).click();
    await expect(page).toHaveURL(/\/nodes$/);
  });
});

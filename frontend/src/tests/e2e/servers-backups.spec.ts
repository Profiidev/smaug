import { expect } from '@playwright/test';
import { HttpResponse, ws } from 'msw';
import * as gen from '$lib/client/msw.gen';
import { test } from '$test_helpers/e2e-fixture';
import { setupSession } from '$test_helpers/session';
import { expectNoHorizontalOverflow, gotoReady } from '$test_helpers/layout';
import { asAccess, ok } from '$test_helpers/servers-e2e';

const BACKUPS = '/servers/srv-survival/backups';

const liveWs = ws.link('*/api/servers/:uuid/ws');

/**
 * Live socket that keeps sending `frame` every 200 ms, like real progress:
 * the tab subscribes after the page opened the socket.
 */
const repeatFrame = (frame: unknown) =>
  liveWs.addEventListener('connection', ({ client }) => {
    const timer = setInterval(() => client.send(JSON.stringify(frame)), 200);
    client.addEventListener('close', () => clearInterval(timer));
  });

const GiB = 1024 ** 3;
const ago = (days: number) =>
  new Date(Date.now() - days * 86_400_000).toISOString();

const backup = (over: Record<string, unknown>) => ({
  created_at: ago(3),
  error: null,
  id: 'b-old',
  ignored: '',
  locked: false,
  name: 'Before 1.21.4 update',
  sha256: `a1f3${'0'.repeat(56)}9c2e`,
  size_bytes: 1.3 * GiB,
  status: 'completed',
  ...over
});

const LOCKED = backup({ locked: true });
const DAILY = backup({
  created_at: ago(0.1),
  id: 'b-daily',
  name: 'Daily',
  sha256: `7be0${'0'.repeat(56)}41d8`,
  size_bytes: 1.2 * GiB
});

const list = (backups: unknown[], used = backups.length, limit = 3) =>
  gen.handleListBackups(
    () => HttpResponse.json({ backups, limit, used }) as never
  );

/** Records JSON request bodies, answering 200. */
const recorder = () => {
  const bodies: unknown[] = [];
  return {
    bodies,
    resolver: async ({ request }: { request: Request }) => {
      bodies.push(await request.json());
      return ok();
    }
  };
};

test.describe('server backups tab', () => {
  test.beforeEach(async ({ context, network }) => {
    await setupSession(context);
    network.use(list([LOCKED, DAILY]));
  });

  test('lists backups with usage and fits the viewport', async ({ page }) => {
    await gotoReady(page, BACKUPS);
    await expect(page.getByRole('heading', { name: 'Backups' })).toBeVisible();
    await expect(page.getByText('2 of 3')).toBeVisible();
    const old = page.getByRole('listitem').filter({ hasText: LOCKED.name });
    await expect(old.getByText('Locked')).toBeVisible();
    await expect(old.getByText('Completed')).toBeVisible();
    await expect(old.getByText('1.3 GiB')).toBeVisible();
    await expect(old.getByText('sha256 a1f3…9c2e')).toBeVisible();
    await expect(old.getByText('3 days ago')).toHaveAttribute('title', /\d/);
    await expect(
      page.getByRole('link', { name: `Download ${LOCKED.name}` })
    ).toHaveAttribute(
      'href',
      '/api/servers/srv-survival/backups/b-old/download'
    );
    // Locked backups can't be deleted.
    await expect(
      page.getByRole('button', { name: `Delete ${LOCKED.name}` })
    ).toBeDisabled();
    await expect(
      page.getByRole('button', { name: 'Delete Daily' })
    ).toBeEnabled();
    await expectNoHorizontalOverflow(page);
  });

  test('creates a backup', async ({ network, page }) => {
    const rec = recorder();
    network.use(
      gen.handleCreateBackup(async (info) => {
        await rec.resolver(info);
        return HttpResponse.json(
          backup({ id: 'b-new', name: 'Pre-update', status: 'creating' })
        ) as never;
      })
    );
    await gotoReady(page, BACKUPS);
    await page.getByRole('button', { name: 'Create backup' }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog.getByLabel('Name')).toHaveAttribute(
      'placeholder',
      /^Backup at \d{4}-\d\d-\d\d \d\d:\d\d$/
    );
    await dialog.getByLabel('Name').fill('Pre-update');
    await dialog.getByLabel('Ignored files').fill('*.log\ncache/');
    await dialog.getByRole('switch', { name: 'Lock this backup' }).click();
    await dialog.getByRole('button', { name: 'Start backup' }).click();
    await expect
      .poll(() => rec.bodies)
      .toEqual([
        { ignored: '*.log\ncache/', locked: true, name: 'Pre-update' }
      ]);
    await expect(page.getByText('Backup Pre-update started')).toBeVisible();
    await expect(dialog).toBeHidden();
  });

  test('sends nulls for an empty create form', async ({ network, page }) => {
    const rec = recorder();
    network.use(gen.handleCreateBackup(rec.resolver));
    await gotoReady(page, BACKUPS);
    await page.getByRole('button', { name: 'Create backup' }).click();
    await page.getByRole('button', { name: 'Start backup' }).click();
    await expect
      .poll(() => rec.bodies)
      .toEqual([{ ignored: null, locked: false, name: null }]);
  });

  test('blocks creating at the limit', async ({ network, page }) => {
    network.use(list([LOCKED, DAILY], 2, 2));
    await gotoReady(page, BACKUPS);
    await expect(page.getByText('Backup limit reached')).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Create backup' })
    ).toBeDisabled();
    await expectNoHorizontalOverflow(page);
  });

  test('toasts a 409 from create', async ({ network, page }) => {
    network.use(
      gen.handleCreateBackup(
        () => new HttpResponse(null, { status: 409 }) as never
      )
    );
    await gotoReady(page, BACKUPS);
    await page.getByRole('button', { name: 'Create backup' }).click();
    await page.getByRole('button', { name: 'Start backup' }).click();
    await expect(
      page.getByText('Backup limit reached', { exact: true })
    ).toBeVisible();
  });

  test('locks and unlocks', async ({ network, page }) => {
    const rec = recorder();
    network.use(gen.handleLockBackup(rec.resolver));
    await gotoReady(page, BACKUPS);
    await page.getByRole('button', { name: 'Lock Daily' }).click();
    await page.getByRole('button', { name: `Unlock ${LOCKED.name}` }).click();
    await expect
      .poll(() => rec.bodies)
      .toEqual([{ locked: true }, { locked: false }]);
  });

  test('deletes after naming the backup and size', async ({
    network,
    page
  }) => {
    let deleted = '';
    network.use(
      gen.handleDeleteBackup(({ params }) => {
        deleted = params.backup;
        return ok();
      })
    );
    await gotoReady(page, BACKUPS);
    await page.getByRole('button', { name: 'Delete Daily' }).click();
    const dialog = page.getByRole('alertdialog');
    await expect(dialog).toContainText('"Daily" (1.2 GiB)');
    // The destructive button never takes initial focus.
    await expect(
      dialog.getByRole('button', { name: 'Delete backup' })
    ).not.toBeFocused();
    await dialog.getByRole('button', { name: 'Delete backup' }).click();
    await expect.poll(() => deleted).toBe('b-daily');
    await expect(page.getByText('Deleted Daily')).toBeVisible();
  });

  test('restores, optionally deleting current files first', async ({
    network,
    page
  }) => {
    const rec = recorder();
    network.use(gen.handleRestoreBackup(rec.resolver));
    await gotoReady(page, BACKUPS);
    await page.getByRole('button', { name: 'Restore Daily' }).click();
    const dialog = page.getByRole('alertdialog');
    await expect(dialog).toContainText('The server is stopped');
    const truncate = dialog.getByRole('checkbox', {
      name: 'Delete all current files first'
    });
    await expect(truncate).not.toBeChecked();
    await expect(
      dialog.getByRole('button', { name: 'Restore backup' })
    ).not.toBeFocused();
    await truncate.click();
    await dialog.getByRole('button', { name: 'Restore backup' }).click();
    await expect.poll(() => rec.bodies).toEqual([{ truncate: true }]);
    await expect(page.getByText('Restoring Daily')).toBeVisible();
  });

  test('shows live progress of a creating backup', async ({
    network,
    page
  }) => {
    network.use(
      list([DAILY, backup({ id: 'b-new', name: 'Fresh', status: 'creating' })]),
      repeatFrame({
        bytes: 512 * 1024 ** 2,
        id: 'b-new',
        status: 'creating',
        type: 'backup'
      })
    );
    await gotoReady(page, BACKUPS);
    const row = page.getByRole('listitem').filter({ hasText: 'Fresh' });
    await expect(row.getByText('Creating')).toBeVisible();
    await expect(row.getByText('512 MiB archived')).toBeVisible();
    await Promise.all(
      ['Download', 'Restore', 'Lock', 'Delete'].map(async (label) =>
        expect(
          row.getByRole('button', { name: `${label} Fresh` })
        ).toBeDisabled()
      )
    );
  });

  test('shows the empty state', async ({ network, page }) => {
    network.use(list([]));
    await gotoReady(page, BACKUPS);
    await expect(page.getByText('No backups yet')).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Create backup' })
    ).toHaveCount(2);
  });

  test('shows an error with retry', async ({ network, page }) => {
    let fail = true;
    network.use(
      gen.handleListBackups(
        () =>
          (fail
            ? new HttpResponse(null, { status: 502 })
            : HttpResponse.json({
                backups: [DAILY],
                limit: 3,
                used: 1
              })) as never
      )
    );
    await gotoReady(page, BACKUPS);
    await expect(page.getByText('Failed to load backups')).toBeVisible();
    fail = false;
    await page.getByRole('button', { name: 'Retry' }).click();
    await expect(page.getByText('Daily', { exact: true })).toBeVisible();
  });

  test('read access hides write actions', async ({ network, page }) => {
    network.use(asAccess('read'));
    await gotoReady(page, BACKUPS);
    await expect(page.getByText('Daily', { exact: true })).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Create backup' })
    ).toHaveCount(0);
    await expect(
      page.getByRole('button', { name: 'Delete Daily' })
    ).toHaveCount(0);
    await expect(
      page.getByRole('link', { name: 'Download Daily' })
    ).toBeVisible();
  });
});

import { type Page, expect } from '@playwright/test';
import { HttpResponse } from 'msw';
import * as gen from '$lib/client/msw.gen';
import { serverInfoOf as serverInfo } from '../mocks/e2e/data';
import { test } from '$test_helpers/e2e-fixture';
import { setupSession } from '$test_helpers/session';
import { expectNoHorizontalOverflow, gotoReady } from '$test_helpers/layout';
import {
  action,
  asAccess,
  liveFrames,
  ok,
  stateFrame
} from '$test_helpers/servers-e2e';

const POWER = ['Start', 'Restart', 'Stop', 'Kill'];
const matrix: [string, string[]][] = [
  ['running', ['Restart', 'Stop', 'Kill']],
  ['starting', ['Stop', 'Kill']],
  ['stopping', ['Kill']],
  ['offline', ['Start']],
  ['crashed', ['Start']],
  ['installing', []],
  ['suspended', []]
];

const stats = {
  cpu_limit_millicores: 2000,
  cpu_millicores: 760,
  disk_bytes: 3.4 * 1024 ** 3,
  disk_limit_bytes: 10 * 1024 ** 3,
  memory_bytes: 2.1 * 1024 ** 3,
  memory_limit_bytes: 4 * 1024 ** 3,
  net_rx_bps: 14_336,
  net_tx_bps: 3072,
  type: 'stats',
  uptime_ms: 273_120_000
};

const command = (page: Page) =>
  page.getByRole('textbox', { name: 'Console command' });

test.describe('server console', () => {
  test.beforeEach(async ({ context }) => setupSession(context));

  test('is the default tab and fits the viewport', async ({ page }) => {
    await gotoReady(page, '/servers/srv-survival');
    await expect(page).toHaveURL(/\/servers\/srv-survival\/console$/);
    await expect(
      page.getByRole('log', { name: 'Console output' })
    ).toBeVisible();
    // Header and Server card both show the primary allocation.
    await expect(page.getByText('10.0.0.1:25565')).toHaveCount(2);
    await expect(page.getByText('Connected')).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Save Changes' })
    ).toBeHidden();
    await expectNoHorizontalOverflow(page);
  });

  for (const [status, on] of matrix) {
    test(`power buttons follow the matrix while ${status}`, async ({
      network,
      page
    }) => {
      network.use(liveFrames(stateFrame(status)));
      await gotoReady(page, '/servers/srv-survival/console');
      await expect(
        page.getByText('Connected').or(page.getByText('Install log'))
      ).toBeVisible();
      for (const name of POWER) {
        // oxlint-disable-next-line no-await-in-loop
        await expect(action(page, name)).toBeEnabled({
          enabled: on.includes(name)
        });
      }
    });
  }

  test('sends power actions and toasts a refusal', async ({
    network,
    page
  }) => {
    const bodies: unknown[] = [];
    let refuse = false;
    network.use(
      liveFrames(stateFrame('running')),
      gen.handleServerPower(async ({ request }) => {
        bodies.push(await request.json());
        return refuse
          ? (new HttpResponse(null, { status: 409 }) as never)
          : ok();
      })
    );
    await gotoReady(page, '/servers/srv-survival/console');
    await expect(action(page, 'Restart')).toBeEnabled();
    await action(page, 'Restart').click();
    await expect.poll(() => bodies).toEqual([{ action: 'restart' }]);
    refuse = true;
    await action(page, 'Kill').click();
    await expect(
      page.getByText("The server can't do that right now.")
    ).toBeVisible();
    expect(bodies).toEqual([{ action: 'restart' }, { action: 'kill' }]);
  });

  test('sends a command on Enter and echoes it', async ({ network, page }) => {
    const bodies: unknown[] = [];
    network.use(
      liveFrames(stateFrame('running'), {
        lines: [
          {
            at: '2026-01-01T12:04:01Z',
            line: 'Server marked as starting...',
            stream: 'daemon'
          },
          {
            at: '2026-01-01T12:04:02Z',
            line: '\u001B[0;36;1m[Server]\u001B[0m Done (8.4s)!\u001B[K',
            stream: 'console'
          }
        ],
        type: 'logs'
      }),
      gen.handleServerCommand(async ({ request }) => {
        bodies.push(await request.json());
        return ok();
      })
    );
    await gotoReady(page, '/servers/srv-survival/console');
    const log = page.getByRole('log', { name: 'Console output' });
    await expect(log).toContainText('[Daemon] Server marked as starting...');
    await expect(log).toContainText('[Server] Done (8.4s)!');
    await expect(log).not.toContainText('[0m');
    await expect(log.getByText('[Server]', { exact: true })).toHaveAttribute(
      'style',
      /color: var\(--ansi-6\)/
    );
    await expect(command(page)).toBeEnabled();
    await command(page).fill('say hi');
    await command(page).press('Enter');
    await expect(log).toContainText('> say hi');
    await expect(command(page)).toHaveValue('');
    expect(bodies).toEqual([{ line: 'say hi' }]);
  });

  test('disables commands while offline', async ({ page }) => {
    await gotoReady(page, '/servers/srv-survival/console');
    await expect(command(page)).toBeDisabled();
    await expect(command(page)).toHaveAttribute(
      'placeholder',
      'Server must be running'
    );
    await expect(
      page.getByRole('button', { name: 'Send command' })
    ).toBeDisabled();
  });

  test('shows live metrics in the card headers', async ({ network, page }) => {
    network.use(liveFrames(stateFrame('running'), stats));
    await gotoReady(page, '/servers/srv-survival/console');
    await expect(page.getByText('760m')).toBeVisible();
    await expect(page.getByText('38% of 2000 millicores')).toBeVisible();
    await expect(page.getByText('53% of 4 GiB limit')).toBeVisible();
    await expect(page.getByText('↓ 14 KB/s')).toBeVisible();
    await expect(page.getByText('34% used')).toBeVisible();
    await expect(page.getByText('3d 3h 52m')).toBeVisible();
    const cpu = page.getByRole('img', { name: 'CPU over the last 60 seconds' });
    await expect(cpu).toBeVisible();
    // Axis in percent of one core.
    await expect(cpu).toContainText('0%');
    await expectNoHorizontalOverflow(page);
  });

  test('read access hides power and locks commands', async ({
    network,
    page
  }) => {
    network.use(asAccess('read'), liveFrames(stateFrame('running')));
    await gotoReady(page, '/servers/srv-survival/console');
    await expect(page.getByText('Connected')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Kill' })).toHaveCount(0);
    await expect(command(page)).toBeDisabled();
    await expect(command(page)).toHaveAttribute(
      'placeholder',
      'Commands need write access'
    );
  });

  for (const [uuid, title] of [
    ['srv-viking', 'Install failed'],
    ['srv-banned', 'This server is suspended'],
    ['srv-creative', 'The server is installing']
  ] as const) {
    test(`${uuid} shows the "${title}" alert`, async ({ page }) => {
      await gotoReady(page, `/servers/${uuid}/console`);
      await expect(
        page.locator('[data-slot=alert-title]', { hasText: title })
      ).toBeVisible();
      await expectNoHorizontalOverflow(page);
    });
  }

  test('labels the install output while installing', async ({
    network,
    page
  }) => {
    network.use(liveFrames(stateFrame('installing')));
    await gotoReady(page, '/servers/srv-creative/console');
    await expect(page.getByText('Install log')).toBeVisible();
    await expect(command(page)).toHaveAttribute(
      'placeholder',
      'Commands are unavailable during install'
    );
    await expect(page.getByText('Power actions are locked.')).toBeVisible();
  });

  test('shows the node error', async ({ network, page }) => {
    network.use(
      gen.handleServerInfo(
        ({ params }) =>
          HttpResponse.json({
            ...serverInfo(params.uuid),
            error: 'Failed to start: no such image'
          }) as never
      )
    );
    await gotoReady(page, '/servers/srv-survival/console');
    await expect(
      page.getByText('Failed to start: no such image')
    ).toBeVisible();
  });

  test('hides Backups without a backup limit', async ({ network, page }) => {
    network.use(
      gen.handleServerInfo(
        ({ params }) =>
          HttpResponse.json({
            ...serverInfo(params.uuid),
            backup_limit: 0
          }) as never
      )
    );
    await gotoReady(page, '/servers/srv-survival/backups');
    await expect(page).toHaveURL(/\/servers\/srv-survival\/console$/);
    await expect(page.getByRole('tab', { name: 'Backups' })).toHaveCount(0);
  });
});

import { expect } from '@playwright/test';
import { HttpResponse, ws } from 'msw';
import * as gen from '$lib/client/msw.gen';
import { test } from '$test_helpers/e2e-fixture';
import { setupSession } from '$test_helpers/session';
import { expectNoHorizontalOverflow, gotoReady } from '$test_helpers/layout';
import { asAccess, fieldErrors, ok } from '$test_helpers/servers-e2e';

const SCHEDULES = '/servers/srv-survival/schedules';

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

const task = (
  id: string,
  position: number,
  action: string,
  payload: string,
  offset_seconds = 0,
  continue_on_failure = false
) => ({
  action,
  continue_on_failure,
  id,
  offset_seconds,
  payload,
  position
});

const NIGHTLY = {
  created_at: '2026-01-01T00:00:00Z',
  cron: {
    day_of_month: '*',
    day_of_week: '*',
    hour: '4',
    minute: '0',
    month: '*'
  },
  enabled: true,
  id: 'sch-nightly',
  last_run_at: '2026-10-20T04:00:00Z',
  name: 'Nightly restart',
  next_run_at: '2026-10-22T04:00:00Z',
  only_when_online: true,
  running_task: null,
  status: 'active',
  tasks: [
    task('t1', 0, 'command', 'say Server restarting in 5 minutes'),
    task('t2', 1, 'command', 'save-all', 300, true),
    task('t3', 2, 'power', 'restart', 10)
  ]
};

const WEEKLY = {
  ...NIGHTLY,
  cron: { ...NIGHTLY.cron, day_of_week: '0', hour: '3' },
  enabled: false,
  id: 'sch-weekly',
  last_run_at: null,
  name: 'Weekly backup',
  next_run_at: null,
  only_when_online: false,
  status: 'inactive',
  tasks: [task('t4', 0, 'backup', '')]
};

const list = (schedules: unknown[]) =>
  gen.handleListSchedules(() => HttpResponse.json(schedules) as never);

const recorder = (respond: () => never = ok) => {
  const bodies: unknown[] = [];
  return {
    bodies,
    resolver: async ({ request }: { request: Request }) => {
      bodies.push(await request.json());
      return respond();
    }
  };
};

test.describe('server schedules tab', () => {
  test.beforeEach(async ({ context, network }) => {
    await setupSession(context);
    network.use(list([NIGHTLY, WEEKLY]));
  });

  test('lists schedules with human cron text', async ({ page }) => {
    await gotoReady(page, SCHEDULES);
    await expect(
      page.getByRole('heading', { name: 'Schedules' })
    ).toBeVisible();
    await expect(
      page.getByText('Every day at 04:00').locator('visible=true')
    ).toBeVisible();
    await expect(
      page.getByText('Every Sunday at 03:00').locator('visible=true')
    ).toBeVisible();
    await expect(
      page.getByText(/Paused/).locator('visible=true')
    ).toBeVisible();
    await expect(
      page.getByText('Inactive', { exact: true }).locator('visible=true')
    ).toBeVisible();
    await expectNoHorizontalOverflow(page);
  });

  test('creates a schedule and maps a 400 to the cron field', async ({
    network,
    page
  }) => {
    let fail = true;
    const created = { ...NIGHTLY, id: 'sch-new', name: 'Hourly save' };
    const rec = recorder();
    network.use(
      gen.handleListSchedules(
        () => HttpResponse.json(fail ? [NIGHTLY] : [NIGHTLY, created]) as never
      ),
      gen.handleCreateSchedule(async (info) => {
        await rec.resolver(info);
        return fail
          ? fieldErrors(400, [
              { field: 'cron.minute', message: 'Invalid minute' }
            ])
          : (HttpResponse.json(created) as never);
      })
    );
    await gotoReady(page, SCHEDULES);
    await page.getByRole('button', { name: 'Create schedule' }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toContainText('UTC');
    await dialog.getByLabel('Name').fill('Hourly save');
    await dialog.getByLabel('Minute').fill('99');
    await dialog.getByRole('button', { name: 'Create schedule' }).click();
    await expect(dialog.getByText('Invalid minute')).toBeVisible();
    expect(rec.bodies).toEqual([
      {
        cron: {
          day_of_month: '*',
          day_of_week: '*',
          hour: '*',
          minute: '99',
          month: '*'
        },
        enabled: true,
        name: 'Hourly save',
        only_when_online: false
      }
    ]);
    fail = false;
    await dialog.getByLabel('Minute').fill('0');
    await dialog.getByRole('button', { name: /Retry|Create schedule/ }).click();
    await expect(page).toHaveURL(/\?id=sch-new$/);
  });

  test('opens the detail with cron boxes and ordered tasks', async ({
    page
  }) => {
    await gotoReady(page, SCHEDULES);
    await page
      .getByRole('link', { name: 'Nightly restart' })
      .locator('visible=true')
      .click();
    await expect(page).toHaveURL(/\?id=sch-nightly$/);
    await expect(
      page.getByText('Every day at 04:00, server time (UTC).')
    ).toBeVisible();
    const tasks = page
      .getByRole('list', { name: 'Tasks' })
      .getByRole('listitem');
    await expect(tasks).toHaveCount(3);
    await expect(tasks.nth(0)).toContainText('Send command');
    await expect(tasks.nth(0)).toContainText('Runs right away');
    await expect(tasks.nth(1)).toContainText('Continues on failure');
    await expect(tasks.nth(1)).toContainText('After 300 s');
    await expect(tasks.nth(2)).toContainText('Send power action');
    await expect(
      page.getByRole('switch', { name: 'Only when the server is online' })
    ).toBeChecked();
    await expectNoHorizontalOverflow(page);
    await page.getByRole('link', { name: 'Back to schedules' }).click();
    await expect(page).toHaveURL(/\/schedules$/);
  });

  test('toggles a switch with a PATCH', async ({ network, page }) => {
    const rec = recorder(() => HttpResponse.json(NIGHTLY) as never);
    network.use(gen.handleUpdateSchedule(rec.resolver));
    await gotoReady(page, `${SCHEDULES}?id=sch-nightly`);
    await page.getByRole('switch', { name: 'Enabled' }).click();
    await expect.poll(() => rec.bodies).toEqual([{ enabled: false }]);
  });

  test('adds a power task', async ({ network, page }) => {
    const rec = recorder(
      () => HttpResponse.json(task('t9', 3, 'power', 'stop')) as never
    );
    network.use(gen.handleCreateScheduleTask(rec.resolver));
    await gotoReady(page, `${SCHEDULES}?id=sch-nightly`);
    await page.getByRole('button', { name: 'Add task' }).click();
    const dialog = page.getByRole('dialog');
    await dialog.getByLabel('Action').click();
    await page.getByRole('option', { name: 'Send power action' }).click();
    await dialog.getByLabel('Power action').click();
    await page.getByRole('option', { exact: true, name: 'stop' }).click();
    await dialog.getByLabel('Time offset (seconds)').fill('30');
    await dialog.getByRole('switch', { name: 'Continue on failure' }).click();
    await dialog.getByRole('button', { name: 'Add task' }).click();
    await expect
      .poll(() => rec.bodies)
      .toEqual([
        {
          action: 'power',
          continue_on_failure: true,
          offset_seconds: 30,
          payload: 'stop'
        }
      ]);
    await expect(page.getByText('Task added')).toBeVisible();
  });

  test('requires a command for command tasks', async ({ page }) => {
    await gotoReady(page, `${SCHEDULES}?id=sch-nightly`);
    await page.getByRole('button', { name: 'Add task' }).click();
    const dialog = page.getByRole('dialog');
    await dialog.getByRole('button', { name: 'Add task' }).click();
    await expect(dialog.getByText('Command is required')).toBeVisible();
  });

  test('toasts why Run now was refused', async ({ network, page }) => {
    network.use(
      gen.handleRunSchedule(
        () => new HttpResponse(null, { status: 409 }) as never
      )
    );
    await gotoReady(page, `${SCHEDULES}?id=sch-nightly`);
    await page.getByRole('button', { name: 'Run now' }).click();
    await expect(
      page.getByText(/only runs while the server is online/)
    ).toBeVisible();
  });

  test('shows the running state from live frames', async ({
    network,
    page
  }) => {
    network.use(
      repeatFrame({
        id: 'sch-nightly',
        status: 'running',
        task_index: 1,
        type: 'schedule'
      })
    );
    await gotoReady(page, `${SCHEDULES}?id=sch-nightly`);
    await expect(page.getByText('Running task 2 of 3')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Run now' })).toBeDisabled();
    await expect(page.getByText('Running now')).toBeVisible();
    const active = page
      .getByRole('list', { name: 'Tasks' })
      .getByRole('listitem')
      .nth(1);
    await expect(active).toHaveAttribute('aria-current', 'step');
    await expect(active).toContainText('Waiting');
  });

  test('deletes a schedule after confirming', async ({ network, page }) => {
    let deleted = '';
    network.use(
      gen.handleDeleteSchedule(({ params }) => {
        deleted = params.schedule;
        return ok();
      })
    );
    await gotoReady(page, `${SCHEDULES}?id=sch-weekly`);
    await page
      .getByRole('tabpanel', { name: 'Schedules' })
      .getByRole('button', { exact: true, name: 'Delete' })
      .click();
    const dialog = page.getByRole('alertdialog');
    await expect(dialog).toContainText('Weekly backup');
    await dialog.getByRole('button', { name: 'Delete schedule' }).click();
    await expect.poll(() => deleted).toBe('sch-weekly');
    await expect(page).toHaveURL(/\/schedules$/);
  });

  test('shows the empty state', async ({ network, page }) => {
    network.use(list([]));
    await gotoReady(page, SCHEDULES);
    await expect(page.getByText('No schedules yet')).toBeVisible();
  });

  test('read access hides write actions', async ({ network, page }) => {
    network.use(asAccess('read'));
    await gotoReady(page, `${SCHEDULES}?id=sch-nightly`);
    await expect(
      page.getByRole('heading', { name: 'Nightly restart' })
    ).toBeVisible();
    await expect(page.getByRole('button', { name: 'Run now' })).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'Add task' })).toHaveCount(0);
    await expect(page.getByRole('switch', { name: 'Enabled' })).toBeDisabled();
  });
});

import { expect } from '@playwright/test';
import { HttpResponse } from 'msw';
import * as gen from '$lib/client/msw.gen';
import { test } from '$test_helpers/e2e-fixture';
import { setupSession } from '$test_helpers/session';
import { gotoReady } from '$test_helpers/layout';
import { action, combo, ok } from '$test_helpers/servers-e2e';
import { serverOptions } from '../mocks/e2e/data';

test.describe('server transfer and delete', () => {
  test.beforeEach(async ({ context }) => setupSession(context));

  test('transfers to another node', async ({ network, page }) => {
    const bodies: unknown[] = [];
    network.use(
      gen.handleTransferServer(async ({ request }) => {
        bodies.push(await request.json());
        return ok();
      })
    );
    await gotoReady(page, '/servers/srv-survival');
    await action(page, 'Transfer…').click();
    const dialog = page.getByRole('dialog');
    await expect(dialog.getByText('Transfer Survival SMP')).toBeVisible();

    // Empty submit validates in the dialog.
    await dialog.getByRole('button', { name: 'Start transfer' }).click();
    await expect(dialog.getByText('Select a target node.')).toBeVisible();
    await expect(
      dialog.getByText('Enter a valid IPv4 or IPv6 address.')
    ).toBeVisible();

    // The current node is not a target.
    await combo(dialog, 'Target node').click();
    await expect(page.getByRole('option', { name: 'Node One' })).toHaveCount(0);
    await page.getByRole('option', { name: 'Node Two' }).click();
    await dialog.getByLabel('IP address').fill(' 10.0.0.2 ');
    await dialog.getByLabel('Port').fill('80');
    await dialog.getByRole('button', { name: 'Start transfer' }).click();
    await expect(
      dialog.getByText('Port must be between 1024 and 65535.')
    ).toBeVisible();

    await dialog.getByLabel('Port').fill('25600');
    await dialog.getByRole('button', { name: 'Start transfer' }).click();
    await expect(
      page.getByText('Server Survival SMP transferred to Node Two')
    ).toBeVisible();
    await expect(dialog).toBeHidden();
    expect(bodies).toEqual([{ ip: '10.0.0.2', node: 'node-2', port: 25_600 }]);
  });

  for (const { body, message, status } of [
    {
      body: { errors: [{ field: 'ip', message: 'IP is not on the node.' }] },
      message: "Couldn't transfer server. Fix the highlighted fields.",
      status: 400
    },
    {
      body: { errors: [{ field: 'port', message: 'Port is taken.' }] },
      message: "Couldn't transfer server: the allocation is already in use.",
      status: 409
    },
    {
      body: { errors: [{ field: 'node', message: 'Node is offline.' }] },
      message: "Couldn't transfer server. Fix the highlighted fields.",
      status: 400
    },
    {
      body: { errors: [{ field: 'other', message: 'Odd transfer.' }] },
      message: 'Odd transfer.',
      status: 400
    },
    { body: undefined, message: "Couldn't transfer server.", status: 500 }
  ]) {
    test(`shows a ${status} transfer error for ${body?.errors[0].field ?? 'no body'}`, async ({
      network,
      page
    }) => {
      network.use(
        gen.handleTransferServer(
          () =>
            (body
              ? HttpResponse.json(body, { status })
              : new HttpResponse(null, { status })) as never
        )
      );
      await gotoReady(page, '/servers/srv-survival');
      await action(page, 'Transfer…').click();
      const dialog = page.getByRole('dialog');
      await combo(dialog, 'Target node').click();
      await page.getByRole('option', { name: 'Node Two' }).click();
      await dialog.getByLabel('IP address').fill('10.0.0.2');
      await dialog.getByRole('button', { name: 'Start transfer' }).click();

      await expect(page.getByText(message).first()).toBeVisible();
      const inline = body?.errors[0];
      if (inline && inline.field !== 'other') {
        await expect(dialog.getByText(inline.message)).toBeVisible();
      }
      await expect(dialog).toBeVisible();
    });
  }

  test('disables transfer with a single node', async ({ network, page }) => {
    network.use(
      gen.handleServerOptions(
        () =>
          HttpResponse.json({
            ...serverOptions.default,
            nodes: [{ name: 'Node One', uuid: 'node-1' }]
          }) as never
      )
    );
    await gotoReady(page, '/servers/srv-survival');
    await expect(action(page, 'Reinstall')).toBeEnabled();
    await expect(action(page, 'Transfer…')).toBeDisabled();
  });

  for (const button of ['Delete', 'Delete server']) {
    test(`deletes from "${button}" and returns to the list`, async ({
      network,
      page
    }) => {
      const deleted: unknown[] = [];
      network.use(
        gen.handleDeleteServer(({ params }) => {
          deleted.push(params.uuid);
          return ok();
        })
      );
      await gotoReady(page, '/servers/srv-survival');
      await action(page, button).click();
      const dialog = page.getByRole('dialog');
      await expect(
        dialog.getByText('Delete server Survival SMP?')
      ).toBeVisible();
      await dialog.getByRole('button', { name: 'Delete' }).click();

      await expect(page).toHaveURL(/\/servers$/);
      await expect(
        page.getByText('Server Survival SMP deleted successfully')
      ).toBeVisible();
      expect(deleted).toEqual(['srv-survival']);
    });
  }

  test('shows a delete failure and stays', async ({ network, page }) => {
    network.use(
      gen.handleDeleteServer(
        () => new HttpResponse(null, { status: 500 }) as never
      )
    );
    await gotoReady(page, '/servers/srv-survival');
    await action(page, 'Delete server').click();
    await page
      .getByRole('dialog')
      .getByRole('button', { name: 'Delete' })
      .click();

    await expect(page.getByText('Failed to delete server')).toBeVisible();
    await expect(page).toHaveURL(/\/servers\/srv-survival$/);
  });
});

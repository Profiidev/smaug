import { type Locator, type Page, expect } from '@playwright/test';
import { HttpResponse } from 'msw';
import * as gen from '$lib/client/msw.gen';
import { serverInfoOf } from '../mocks/e2e/data';

export const ok = () => new HttpResponse(null, { status: 200 }) as never;

export const fieldErrors = (
  status: number,
  errors: { field: string; message: string }[]
) => HttpResponse.json({ errors }, { status }) as never;

export const row = (page: Page, name: string) =>
  page.getByRole('row', { name: new RegExp(name) });

export const next = async (page: Page) =>
  page.getByRole('button', { name: 'Next' }).click();

export const previous = async (page: Page) =>
  page.getByRole('button', { name: 'Previous' }).click();

/**
 * Runs `fill`, submits the stage and waits for `expected`. Retried: right
 * after an invalid submit superforms can still write the submitted values
 * back, swallowing fills made in that window (seen on Mobile Chrome).
 */
export const submitWith = async (
  page: Page,
  fill: () => Promise<unknown>,
  expected: Locator
) =>
  expect(async () => {
    await fill();
    await next(page);
    await expect(expected).toBeVisible({ timeout: 1000 });
  }).toPass();

export const stageTitle = (page: Page, title: string) =>
  page.locator('[data-slot=card-title]', { hasText: title });

/**
 * The FormSelect trigger labelled `label`. Its label is not linked to the
 * combobox (no accessible name), so locate it next to the label text.
 */
export const combo = (scope: Page | Locator, label: string) =>
  scope
    .getByText(label, { exact: true })
    .locator('xpath=following-sibling::*[@role="combobox"]');

/** The FormSelect trigger inside the group named `name` (grant rows, e.g. `User 1`). */
export const rowCombo = (scope: Page | Locator, name: string) =>
  scope.getByRole('group', { exact: true, name }).getByRole('combobox');

/** Picks `option` in the single FormSelect labelled `label`. */
export const pick = async (page: Page, label: string, option: string) => {
  await combo(page, label).click();
  await page.getByRole('option', { name: option }).click();
};

/** Opens the bits-ui Select `trigger` and picks `option`. */
export const choose = async (page: Page, trigger: Locator, option: string) => {
  await trigger.click();
  await page.getByRole('option', { exact: true, name: option }).click();
};

/** Fills the General stage with valid values (owner is prefilled). */
export const fillGeneral = async (page: Page, egg = 'Paper') => {
  await page.getByLabel('Name', { exact: true }).fill('Survival');
  await pick(page, 'Node', 'Node One');
  await pick(page, 'Egg', egg);
  await page.getByLabel('IP address').first().fill('0.0.0.0');
};

export const tab = (page: Page, name: string) =>
  page.getByRole('tab', { name: new RegExp(`^${name}`) });

export const openTab = async (page: Page, name: string) =>
  tab(page, name).click();

export const save = async (page: Page) =>
  page.getByRole('button', { name: 'Save Changes' }).click();

/** Serves `serverInfo` with `access` for every server. */
export const asAccess = (access: 'read' | 'write' | 'admin') =>
  gen.handleServerInfo(
    ({ params }) =>
      HttpResponse.json(serverInfoOf(params.uuid, access)) as never
  );

/** Records `updateServer` bodies and answers with `respond` (default 200). */
export const recordUpdate = (respond: () => never = ok) => {
  const bodies: unknown[] = [];
  return {
    bodies,
    handler: gen.handleUpdateServer(async ({ request }) => {
      bodies.push(await request.json());
      return respond();
    })
  };
};

/** `PUT` body of srv-survival saved unchanged by an admin. */
export const survivalBody = {
  allocations: [
    { ip: '10.0.0.1', port: 25_565 },
    { ip: '10.0.0.1', port: 25_566 },
    { ip: '10.0.0.1', port: 25_567 }
  ],
  backup_limit: 2,
  egg: 'egg-paper',
  environment: {
    MAX_PLAYERS: '20',
    SERVER_JARFILE: 'server.jar',
    VERSION: 'latest'
  },
  grants: {
    groups: [{ access: 'read', uuid: 'group-staff' }],
    users: [{ access: 'write', uuid: 'user-1' }]
  },
  image: 'ghcr.io/pelican-eggs/yolks:java_21',
  limits: { cpu: 2000, disk: 0, memory: 4096 },
  name: 'Survival SMP',
  owner: 'user-admin',
  restart_on_crash: true,
  startup: 'java -jar {{SERVER_JARFILE}}',
  startup_overridden: false,
  stop_timeout: 60
};

export const action = (page: Page, name: string) =>
  page.getByRole('button', { exact: true, name });

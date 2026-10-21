import { type Page, expect } from '@playwright/test';
import { HttpResponse } from 'msw';
import * as gen from '$lib/client/msw.gen';
import { eggInfo } from '../mocks/e2e/data';

/** Records the JSON body of every request a handler sees. */
export const recorder = () => {
  const bodies: Record<string, unknown>[] = [];
  return {
    bodies,
    record: async (request: Request) => {
      bodies.push((await request.json()) as Record<string, unknown>);
    }
  };
};

export const openTab = async (page: Page, name: string) =>
  page.getByRole('tab', { name }).click();

export const badRequest = (message: string) =>
  new HttpResponse(message, { status: 400 }) as never;

export const plcnFile = {
  ...eggInfo,
  _comment: 'DO NOT EDIT: FILE GENERATED AUTOMATICALLY BY PANEL',
  config: {
    files: JSON.stringify(eggInfo.config.files),
    logs: '{}',
    startup: JSON.stringify(eggInfo.config.startup),
    stop: 'stop'
  },
  exported_at: '2026-01-01T00:00:00+00:00',
  meta: { update_url: null, version: 'PLCN_v3' },
  name: 'Imported Paper',
  uuid: '9eec81aa-b165-4233-aa30-1244c6c96e9f'
};

export const ptdlFile = {
  _comment:
    'DO NOT EDIT: FILE GENERATED AUTOMATICALLY BY PTERODACTYL PANEL - PTERODACTYL.IO',
  author: 'dev@example.com',
  config: plcnFile.config,
  description: null,
  docker_images: eggInfo.docker_images,
  exported_at: '2024-01-01T00:00:00+00:00',
  features: [],
  file_denylist: [],
  meta: { update_url: null, version: 'PTDL_v2' },
  name: 'Legacy Paper',
  scripts: eggInfo.scripts,
  startup: 'java -jar server.jar',
  variables: []
};

/** Egg source with a new description and an extra install line. */
export const changedSource = {
  ...eggInfo,
  description: 'Paper, now faster',
  scripts: {
    installation: {
      ...eggInfo.scripts.installation,
      script: 'echo install\necho updated'
    }
  }
};

export const sourceHandler = (sources: Record<string, unknown>) =>
  gen.handleEggSource(({ params }) =>
    params.uuid in sources
      ? (HttpResponse.json({ source: sources[params.uuid] }) as never)
      : (new HttpResponse('Could not reach the update URL', {
          status: 502
        }) as never)
  );

/** Reviews the open update dialog and applies the preselected sections. */
export const applyUpdate = async (page: Page) => {
  const dialog = page.getByRole('dialog');
  await expect(dialog.getByText('Update Paper')).toBeVisible();
  await expect(dialog.getByRole('checkbox', { name: 'General' })).toBeChecked();
  await expect(
    dialog.getByRole('checkbox', { name: 'Install script' })
  ).toBeChecked();
  await expect(
    dialog.getByRole('checkbox', { name: 'Variables' })
  ).toBeDisabled();
  await expect(dialog.getByText('+1 line, −0 lines.')).toBeVisible();
  await expect(dialog.getByText('echo updated')).toBeVisible();
  await dialog.getByRole('button', { name: 'Update 2 sections' }).click();
  await expect(page.getByText('Egg Paper updated')).toBeVisible();
  await expect(dialog).toBeHidden();
};

export const notSquashed = async (page: Page) =>
  page
    .locator('[data-slot="card"] [data-slot="card"]')
    .evaluateAll((cards) =>
      cards.every((el) => el.scrollHeight <= el.clientHeight + 1)
    );

export const pageScrolls = async (page: Page) =>
  page.evaluate(
    () =>
      document.scrollingElement!.scrollHeight >
      document.scrollingElement!.clientHeight
  );

export const next = async (page: Page) =>
  page.getByRole('button', { name: 'Next' }).click();

export const importEgg = async (page: Page, egg: object = plcnFile) => {
  await page.getByLabel(/Import egg JSON/).setInputFiles({
    buffer: Buffer.from(JSON.stringify(egg)),
    mimeType: 'application/json',
    name: 'egg-paper.json'
  });
  await expect(page.getByText('Imported egg-paper.json')).toBeVisible();
};

/** Clicks through the remaining wizard stages and submits. */
export const finishWizard = async (page: Page) => {
  // Each stage is confirmed by one of its own controls.
  for (const ready of [
    page.getByRole('button', { name: 'Add variable' }),
    page.getByLabel('Install Container Image'),
    page.getByLabel('Stop Command'),
    page.getByRole('button', { name: 'Add config file' })
  ]) {
    // oxlint-disable-next-line no-await-in-loop
    await next(page);
    // oxlint-disable-next-line no-await-in-loop
    await expect(ready).toBeVisible();
  }
  await page.getByRole('button', { exact: true, name: 'Create' }).click();
};

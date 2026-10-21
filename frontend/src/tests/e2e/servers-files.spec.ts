import { type Page, expect } from '@playwright/test';
import { HttpResponse } from 'msw';
import * as gen from '$lib/client/msw.gen';
import { test } from '$test_helpers/e2e-fixture';
import { setupSession } from '$test_helpers/session';
import { expectNoHorizontalOverflow, gotoReady } from '$test_helpers/layout';
import { asAccess, ok } from '$test_helpers/servers-e2e';

const FILES = '/servers/srv-survival/files';
const modified = new Date(Date.now() - 2 * 86_400_000).toISOString();
const entry = (name: string, kind: 'file' | 'dir', size = 0) => ({
  kind,
  mime: null,
  mode: 0o644,
  modified,
  name,
  size
});

const tree: Record<string, ReturnType<typeof entry>[]> = {
  '/': [
    entry('server.properties', 'file', 1200),
    entry('plugins', 'dir'),
    entry('server.jar', 'file', 40 * 1024 * 1024),
    entry('readme.txt', 'file', 412),
    entry('world', 'dir')
  ],
  '/empty': [],
  '/plugins': [entry('config.yml', 'file', 64), entry('Essentials', 'dir')]
};
const CONFIG = 'settings:\n  debug: false\n';

/** Records JSON request bodies, answering 200. */
const recorder = () => {
  const bodies: unknown[] = [];
  const resolver = async ({ request }: { request: Request }) => {
    bodies.push(await request.json());
    return ok();
  };
  return { bodies, resolver };
};

const listHandler = gen.handleListFiles(({ request }) => {
  const path = new URL(request.url).searchParams.get('path') ?? '/';
  const items = tree[path];
  return (
    items ? HttpResponse.json(items) : new HttpResponse(null, { status: 404 })
  ) as never;
});

const name = (page: Page, text: string) =>
  page.getByText(text, { exact: true }).locator('visible=true');

test.describe('server files tab', () => {
  test.beforeEach(async ({ context, network }) => {
    await setupSession(context);
    network.use(
      listHandler,
      gen.handleFileContent(
        () => new HttpResponse(CONFIG, { status: 200 }) as never
      )
    );
  });

  test('lists folders first with sizes and fits the viewport', async ({
    page
  }) => {
    await gotoReady(page, FILES);
    const names = page
      .locator('a, span')
      .filter({ hasText: /^(?:plugins|world|readme\.txt|server\.jar)$/ })
      .locator('visible=true');
    await expect(names).toHaveText([
      'plugins',
      'world',
      'readme.txt',
      'server.jar'
    ]);
    await expect(page.getByText('5 items · 40.0 MiB')).toBeVisible();
    await expect(page.getByText('/home/container')).toBeVisible();
    // Binary files download only, text files are editable too.
    await expect(
      page.getByRole('link', { name: 'Edit server.jar' })
    ).toHaveCount(0);
    await expect(
      page.getByRole('link', { name: 'Download server.jar' })
    ).toHaveAttribute(
      'href',
      '/api/servers/srv-survival/files/download?path=%2Fserver.jar'
    );
    await expect(
      page.getByRole('link', { name: 'Edit readme.txt' })
    ).toBeVisible();
    await expectNoHorizontalOverflow(page);
  });

  test('filters the folder by search', async ({ page }) => {
    await gotoReady(page, FILES);
    await page.getByRole('textbox', { name: 'Search files' }).fill('read');
    await expect(name(page, 'readme.txt')).toBeVisible();
    await expect(name(page, 'plugins')).toHaveCount(0);
    await expect(page.getByText('1 item · 412 B')).toBeVisible();
  });

  test('navigates into a folder and back via the breadcrumb', async ({
    page
  }) => {
    await gotoReady(page, FILES);
    await page.getByRole('link', { exact: true, name: 'plugins' }).click();
    await expect(page).toHaveURL(/\?path=\/plugins$/);
    await expect(name(page, 'config.yml')).toBeVisible();
    await expect(
      page.getByRole('navigation', { name: 'Path' }).getByText('plugins')
    ).toHaveAttribute('aria-current', 'page');
    await page.getByRole('link', { name: '/home/container' }).click();
    await expect(name(page, 'readme.txt')).toBeVisible();
  });

  test('shows the empty state', async ({ page }) => {
    await gotoReady(page, `${FILES}?path=/empty`);
    await expect(page.getByText('This folder is empty')).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Upload files' })
    ).toBeVisible();
  });

  test('shows an error with retry', async ({ page, network }) => {
    let fail = true;
    network.use(
      gen.handleListFiles(
        () =>
          (fail
            ? new HttpResponse(null, { status: 502 })
            : HttpResponse.json(tree['/'])) as never
      )
    );
    await gotoReady(page, FILES);
    await expect(page.getByText('Failed to load files')).toBeVisible();
    fail = false;
    await page.getByRole('button', { name: 'Retry' }).click();
    await expect(name(page, 'readme.txt')).toBeVisible();
  });

  test('renames with a relative path', async ({ page, network }) => {
    const rename = recorder();
    network.use(gen.handleRenameFile(rename.resolver));
    await gotoReady(page, FILES);
    await page.getByRole('button', { name: 'Actions for readme.txt' }).click();
    await page.getByRole('menuitem', { name: 'Rename' }).click();
    const input = page.getByRole('dialog').getByLabel('Name');
    await expect(input).toHaveValue('readme.txt');
    await input.fill('../plugins/moved.txt');
    await page.getByRole('button', { name: 'Rename' }).click();
    await expect
      .poll(() => rename.bodies)
      .toEqual([{ from: '/readme.txt', to: '/plugins/moved.txt' }]);
  });

  test('shows a conflict on the name field', async ({ page, network }) => {
    network.use(
      gen.handleCreateFolder(
        () => new HttpResponse(null, { status: 409 }) as never
      )
    );
    await gotoReady(page, FILES);
    await page.getByRole('button', { name: 'New folder' }).click();
    await page.getByRole('dialog').getByLabel('Name').fill('plugins');
    await page.getByRole('button', { name: 'Create' }).click();
    await expect(
      page.getByText('Something with this name already exists')
    ).toBeVisible();
  });

  test('bulk deletes after confirmation', async ({ page, network }) => {
    const del = recorder();
    network.use(gen.handleDeleteFiles(del.resolver));
    await gotoReady(page, FILES);
    await page.getByRole('checkbox', { name: 'Select world' }).click();
    await page.getByRole('checkbox', { name: 'Select readme.txt' }).click();
    await expect(page.getByText('2 items selected')).toBeVisible();
    await page
      .getByRole('tabpanel', { name: 'Files' })
      .getByRole('button', { exact: true, name: 'Delete' })
      .click();

    const dialog = page.getByRole('alertdialog');
    await expect(dialog.getByText('Delete 2 items?')).toBeVisible();
    await expect(
      dialog.getByText('Folders are deleted with everything inside them.')
    ).toBeVisible();
    // The destructive action never takes initial focus.
    await expect(
      dialog.getByRole('button', { name: 'Delete' })
    ).not.toBeFocused();
    await dialog.getByRole('button', { name: 'Delete' }).click();
    await expect
      .poll(() => del.bodies)
      .toEqual([{ paths: ['/world', '/readme.txt'] }]);
    await expect(page.getByText('2 items selected')).toHaveCount(0);
  });

  test('edits and saves a file with Ctrl+S', async ({ page, network }) => {
    const saved: string[] = [];
    network.use(
      gen.handleWriteFileContent(async ({ request }) => {
        saved.push(await request.text());
        return ok();
      })
    );
    await gotoReady(page, `${FILES}?path=/plugins`);
    await page.getByRole('link', { name: 'Edit config.yml' }).click();
    await expect(page).toHaveURL(/\?path=\/plugins&edit=config\.yml$/);

    const editor = page.getByRole('textbox', {
      name: 'Contents of config.yml'
    });
    await expect(editor).toContainText('debug: false');
    await expect(page.getByText('YAML · UTF-8 · LF')).toBeVisible();
    const save = page.getByRole('button', { name: 'Save content' });
    await expect(save).toBeDisabled();

    await editor.click();
    await page.keyboard.press('ControlOrMeta+End');
    await page.keyboard.type('motd: hi');
    await expect(page.getByText('Unsaved changes')).toBeVisible();
    await page.keyboard.press('ControlOrMeta+s');
    await expect.poll(() => saved).toEqual([`${CONFIG}motd: hi`]);
    await expect(page.getByText('Unsaved changes')).toHaveCount(0);
    await expect(save).toBeDisabled();

    await page.getByRole('link', { name: 'Files' }).click();
    await expect(name(page, 'Essentials')).toBeVisible();
  });

  test('discards edits and warns before leaving', async ({ page }) => {
    await gotoReady(page, `${FILES}?path=/plugins&edit=config.yml`);
    const editor = page.getByRole('textbox', {
      name: 'Contents of config.yml'
    });
    await editor.click();
    await page.keyboard.type('x');
    await expect(page.getByText('Unsaved changes')).toBeVisible();

    page.once('dialog', async (d) => d.dismiss());
    await page.getByRole('link', { name: 'Files' }).click();
    await expect(page).toHaveURL(/edit=config\.yml/);

    await page.getByRole('button', { name: 'Discard' }).click();
    await expect(page.getByText('Unsaved changes')).toHaveCount(0);
    await expect(editor).toHaveText(CONFIG.replace(/\n/g, ''));
  });
});

test.describe('server files tab (read access)', () => {
  test.beforeEach(async ({ context, network }) => {
    await setupSession(context);
    network.use(listHandler, asAccess('read'));
  });

  test('hides write actions', async ({ page }) => {
    await gotoReady(page, FILES);
    await expect(name(page, 'readme.txt')).toBeVisible();
    await Promise.all(
      ['New file', 'New folder', 'Upload'].map(async (label) =>
        expect(page.getByRole('button', { name: label })).toHaveCount(0)
      )
    );
    await expect(page.getByRole('checkbox')).toHaveCount(0);
    await expect(
      page.getByRole('button', { name: 'Actions for readme.txt' })
    ).toHaveCount(0);
    await expect(
      page.getByRole('link', { name: 'Download readme.txt' })
    ).toBeVisible();
  });
});

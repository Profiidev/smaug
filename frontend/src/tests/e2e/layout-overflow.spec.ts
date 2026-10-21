import type { Locator, Page } from '@playwright/test';
import { test } from '$test_helpers/e2e-fixture';
import { setupSession } from '$test_helpers/session';
import { expectPageFits } from '$test_helpers/layout';

/**
 * Systematic horizontal-overflow sweep. Playwright runs every test across the
 * desktop and mobile projects from `playwright.config.ts`, so each entry below
 * is checked on every viewport.
 */
const pages: { path: string; ready: (page: Page) => Locator }[] = [
  { path: '/', ready: (p) => p.getByRole('main').getByText('Overview') },

  { path: '/nodes', ready: (p) => p.getByRole('heading', { name: 'Nodes' }) },
  {
    path: '/nodes/create',
    ready: (p) => p.getByPlaceholder('Enter name')
  },
  {
    path: '/nodes/node-1/setup',
    ready: (p) => p.getByRole('heading', { name: /Node:/ })
  },
  {
    path: '/nodes/node-1/general',
    ready: (p) => p.getByPlaceholder('Enter name')
  },
  {
    path: '/nodes/node-1/resources',
    ready: (p) => p.getByRole('switch', { name: 'Unlimited CPU' })
  },

  { path: '/eggs', ready: (p) => p.getByRole('heading', { name: 'Eggs' }) },
  { path: '/eggs/create', ready: (p) => p.getByLabel('Egg Name') },
  {
    path: '/eggs/egg-paper/general',
    ready: (p) => p.getByLabel('Egg Name')
  },
  {
    path: '/eggs/egg-paper/variables',
    ready: (p) => p.getByRole('button', { name: 'Add variable' })
  },
  {
    path: '/eggs/egg-paper/install',
    ready: (p) => p.getByLabel('Install Container Image')
  },
  {
    path: '/eggs/egg-paper/runtime',
    ready: (p) => p.getByRole('button', { name: 'Add image' })
  },
  {
    path: '/eggs/egg-paper/config',
    ready: (p) => p.getByLabel('File Path')
  },

  {
    path: '/servers',
    ready: (p) => p.getByRole('heading', { name: 'Servers' })
  },
  {
    path: '/servers/create',
    ready: (p) => p.getByLabel('Name', { exact: true })
  },
  {
    path: '/servers/srv-survival',
    ready: (p) => p.getByRole('heading', { name: /Server:/ })
  },

  { path: '/users', ready: (p) => p.getByRole('heading', { name: 'Users' }) },
  {
    path: '/users/create',
    ready: (p) => p.getByPlaceholder('Enter user name')
  },
  {
    path: '/users/user-1',
    ready: (p) => p.getByRole('heading', { name: /User:/ })
  },

  { path: '/groups', ready: (p) => p.getByRole('heading', { name: 'Groups' }) },
  {
    path: '/groups/create',
    ready: (p) => p.getByPlaceholder('Enter group name')
  },
  {
    path: '/groups/group-admins',
    ready: (p) => p.getByRole('heading', { name: /Group:/ })
  },

  {
    path: '/settings/user',
    ready: (p) => p.getByRole('heading', { name: 'User Settings' })
  },
  {
    path: '/settings/mail',
    ready: (p) => p.getByRole('heading', { name: 'Mail Settings' })
  },
  {
    path: '/account/general',
    ready: (p) => p.getByRole('heading', { name: 'General Settings' })
  },
  {
    path: '/account/auth',
    ready: (p) => p.getByRole('heading', { name: 'Authentication' })
  }
];

test.describe('no horizontal overflow', () => {
  test.beforeEach(async ({ context }) => setupSession(context));

  for (const { path, ready } of pages) {
    test(`${path} fits its viewport`, async ({ page }) => {
      await expectPageFits(page, path, ready(page));
    });
  }
});

import { expect } from '@playwright/test';
import { test } from '$test_helpers/e2e-fixture';
import { setupSession } from '$test_helpers/session';
import { gotoReady } from '$test_helpers/layout';

test.beforeEach(async ({ context }) => setupSession(context));

// Detail pages redirect back to their list with `?error=...` when a record
// Fails to load; the list surfaces that as a toast and strips the param.
const cases = [
  { error: 'not_found', message: 'User not found', path: '/users' },
  { error: 'other', message: 'Failed to load user', path: '/users' },
  { error: 'not_found', message: 'Group not found', path: '/groups' },
  { error: 'other', message: 'Failed to load group', path: '/groups' },
  { error: 'not_found', message: 'Node not found', path: '/nodes' },
  { error: 'other', message: 'Failed to load node', path: '/nodes' },
  { error: 'not_found', message: 'Egg not found', path: '/eggs' },
  { error: 'other', message: 'Failed to load egg', path: '/eggs' }
];

test.describe('list error toasts', () => {
  for (const { error, message, path } of cases) {
    test(`shows "${message}" on ${path}?error=${error}`, async ({ page }) => {
      await gotoReady(page, `${path}?error=${error}`);

      await expect(page.getByText(message)).toBeVisible();
      await expect(page).toHaveURL(new RegExp(`${path}$`));
    });
  }
});

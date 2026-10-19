import { describe, expect, it } from 'vitest';
import { render } from '@testing-library/svelte';
import Harness from '$test_helpers/GroupPermissionsHarness.svelte';
import { Permission } from '$lib/permissions.svelte';
import type { UserInfo } from '$lib/client';

const allPermissions = Object.values(Permission);
const userWith = (permissions: string[]) =>
  ({ permissions }) as unknown as UserInfo;

/** The form keys (`resource$action`) of the rendered permission checkboxes. */
const shownKeys = (container: HTMLElement) =>
  [...container.querySelectorAll<HTMLInputElement>('input[type=checkbox]')]
    .map((i) => i.name)
    .toSorted();

/** The visible checkbox toggles (the hidden inputs are excluded). */
const toggles = (container: HTMLElement) => [
  ...container.querySelectorAll('button[role=checkbox]')
];

const keyOf = (p: string) => p.replace(':', '$');

describe('group Permissions', () => {
  it('lists every Permission for a user holding all of them', () => {
    const { container } = render(Harness, { user: userWith(allPermissions) });
    expect(shownKeys(container)).toEqual(allPermissions.map(keyOf).toSorted());
  });

  it('only lists the permissions the current user holds', () => {
    const { container } = render(Harness, {
      user: userWith([Permission.NODE_VIEW, Permission.USER_EDIT])
    });
    expect(shownKeys(container)).toEqual(['node$view', 'user$edit']);
  });

  it('lists nothing without a user', () => {
    const { container } = render(Harness, {});
    expect(shownKeys(container)).toEqual([]);
  });

  it('disables every checkbox when readonly', () => {
    const { container } = render(Harness, {
      readonly: true,
      user: userWith(allPermissions)
    });
    const boxes = toggles(container);
    expect(boxes).toHaveLength(allPermissions.length);
    for (const box of boxes) {
      expect(box).toBeDisabled();
    }
  });

  it('enables the checkboxes when editable', () => {
    const { container } = render(Harness, { user: userWith(allPermissions) });
    for (const box of toggles(container)) {
      expect(box).toBeEnabled();
    }
  });
});

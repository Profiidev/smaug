import type { ParamMatcher } from '@sveltejs/kit';

/** Server page tabs, in display order. */
export const SERVER_TABS = [
  'console',
  'files',
  'backups',
  'schedules',
  'general',
  'startup',
  'variables',
  'resources',
  'permissions',
  'actions'
] as const;

export type ServerTab = (typeof SERVER_TABS)[number];

export const match = ((param: string): param is ServerTab =>
  (SERVER_TABS as readonly string[]).includes(param)) satisfies ParamMatcher;

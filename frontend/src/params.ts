import { defineParams } from '@sveltejs/kit/params';

const matchEggtab = (
  param: string
): param is 'general' | 'variables' | 'install' | 'runtime' | 'config' =>
  ['general', 'variables', 'install', 'runtime', 'config'].includes(param);

const matchNodetab = (
  param: string
): param is 'setup' | 'general' | 'resources' =>
  ['setup', 'general', 'resources'].includes(param);

/** Server page tabs, in display order. */
const SERVER_TABS = [
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

type ServerTab = (typeof SERVER_TABS)[number];

const matchServertab = (param: string): param is ServerTab =>
  (SERVER_TABS as readonly string[]).includes(param);

export const params = defineParams({
  eggtab: (param) => (matchEggtab(param) ? param : undefined),
  nodetab: (param) => (matchNodetab(param) ? param : undefined),
  servertab: (param) => (matchServertab(param) ? param : undefined)
});

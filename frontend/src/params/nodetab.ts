import type { ParamMatcher } from '@sveltejs/kit';

export const match = ((
  param: string
): param is 'setup' | 'general' | 'resources' =>
  ['setup', 'general', 'resources'].includes(param)) satisfies ParamMatcher;

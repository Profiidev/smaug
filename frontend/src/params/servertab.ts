import type { ParamMatcher } from '@sveltejs/kit';

export const match = ((
  param: string
): param is
  | 'status'
  | 'general'
  | 'startup'
  | 'variables'
  | 'resources'
  | 'permissions' =>
  [
    'status',
    'general',
    'startup',
    'variables',
    'resources',
    'permissions'
  ].includes(param)) satisfies ParamMatcher;

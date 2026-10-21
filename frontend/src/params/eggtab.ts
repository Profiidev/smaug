import type { ParamMatcher } from '@sveltejs/kit';

export const match = ((
  param: string
): param is 'general' | 'variables' | 'install' | 'runtime' | 'config' =>
  ['general', 'variables', 'install', 'runtime', 'config'].includes(
    param
  )) satisfies ParamMatcher;

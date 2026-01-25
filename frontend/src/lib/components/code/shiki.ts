// Follows the best practices established in https://shiki.matsu.io/guide/best-performance
import { createJavaScriptRegexEngine } from 'shiki/engine/javascript';
import { createHighlighterCore } from 'shiki/core';

const bundledLanguages = {
  bash: () => import('@shikijs/langs/bash'),
  yaml: () => import('@shikijs/langs/yaml')
};

/** The languages configured for the highlighter */
export type SupportedLanguage = keyof typeof bundledLanguages | 'text';

/** A preloaded highlighter instance. */
export const highlighter = createHighlighterCore({
  themes: [
    import('@shikijs/themes/github-light-default'),
    import('@shikijs/themes/github-dark-default')
  ],
  langs: Object.entries(bundledLanguages).map(([_, lang]) => lang),
  engine: createJavaScriptRegexEngine()
});

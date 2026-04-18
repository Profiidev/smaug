import { Context } from 'runed';
import type { ReadableBoxedValues, WritableBoxedValues } from 'svelte-toolbelt';
import type { CodeRootProps } from './types';
import { highlighter } from './shiki';
import DOMPurify from 'isomorphic-dompurify';
import type { HighlighterCore } from 'shiki';

type CodeOverflowStateProps = WritableBoxedValues<{
  collapsed: boolean;
}>;

class CodeOverflowState {
  constructor(readonly opts: CodeOverflowStateProps) {
    this.toggleCollapsed = this.toggleCollapsed.bind(this);
  }

  toggleCollapsed() {
    this.opts.collapsed.current = !this.opts.collapsed.current;
  }

  get collapsed() {
    return this.opts.collapsed.current;
  }
}

type CodeRootStateProps = ReadableBoxedValues<{
  code: string;
  lang: NonNullable<CodeRootProps['lang']>;
  hideLines: boolean;
  highlight: CodeRootProps['highlight'];
}>;

class CodeRootState {
  highlighter: HighlighterCore | null = $state(null); // oxlint-disable-line no-null

  constructor(
    readonly opts: CodeRootStateProps,
    readonly overflow?: CodeOverflowState
  ) {
    const _ = highlighter.then((hl) => (this.highlighter = hl));
  }

  highlight(code: string) {
    return this.highlighter?.codeToHtml(code, {
      lang: this.opts.lang.current,
      themes: {
        dark: 'github-dark-default',
        light: 'github-light-default'
      },
      transformers: [
        {
          line: (node, line) => {
            if (within(line, this.opts.highlight.current)) {
              node.properties.class += ' line--highlighted';
            }

            return node;
          },
          pre: (el) => {
            el.properties.style = '';

            if (!this.opts.hideLines.current) {
              el.properties.class += ' line-numbers';
            }

            return el;
          }
        }
      ]
    });
  }

  get code() {
    return this.opts.code.current;
  }

  highlighted = $derived(DOMPurify.sanitize(this.highlight(this.code) ?? ''));
}

const within = (num: number, range: CodeRootProps['highlight']) => {
  if (!range) {
    return false;
  }

  let isWithin = false;

  for (const r of range) {
    if (typeof r === 'number') {
      if (num === r) {
        isWithin = true;
        break;
      }
      continue;
    }

    if (r[0] <= num && num <= r[1]) {
      isWithin = true;
      break;
    }
  }

  return isWithin;
};

class CodeCopyButtonState {
  constructor(readonly root: CodeRootState) {}

  get code() {
    return this.root.opts.code.current;
  }
}

const overflowCtx = new Context<CodeOverflowState>('code-overflow-state');

const ctx = new Context<CodeRootState>('code-root-state');

export const useCodeOverflow = (props: CodeOverflowStateProps) => overflowCtx.set(new CodeOverflowState(props));

export const useCode = (props: CodeRootStateProps) => ctx.set(new CodeRootState(props, overflowCtx.getOr(undefined)));

export const useCodeCopyButton = () => new CodeCopyButtonState(ctx.get());

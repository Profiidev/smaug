import type { WithChildren, WithoutChildren } from 'bits-ui';
import type { CodeVariant } from '.';
import type { SupportedLanguage } from './shiki';
import type { HTMLAttributes } from 'svelte/elements';
import type { ButtonProps } from '@profidev/pleiades/components/ui/button';
import type { Snippet } from 'svelte';

export type CopyButtonPropsWithoutHTML = WithChildren<
  Pick<ButtonProps, 'size' | 'variant'> & {
    ref?: HTMLButtonElement | null;
    text: string;
    icon?: Snippet;
    animationDuration?: number;
    onCopy?: (status: 'success' | 'failure' | undefined) => void;
  }
>;

export type CodeRootPropsWithoutHTML = WithChildren<{
  ref?: HTMLDivElement | null;
  variant?: CodeVariant;
  lang?: SupportedLanguage;
  code: string;
  class?: string;
  hideLines?: boolean;
  highlight?: (number | [number, number])[];
}>;

export type CodeRootProps = CodeRootPropsWithoutHTML &
  WithoutChildren<HTMLAttributes<HTMLDivElement>>;

export type CodeCopyButtonPropsWithoutHTML = Omit<
  CopyButtonPropsWithoutHTML,
  'text'
>;

export type CodeCopyButtonProps = CodeCopyButtonPropsWithoutHTML &
  WithoutChildren<HTMLAttributes<HTMLButtonElement>>;

export type CodeOverflowPropsWithoutHTML = WithChildren<{
  collapsed?: boolean;
}>;

export type CodeOverflowProps = CodeOverflowPropsWithoutHTML &
  WithoutChildren<HTMLAttributes<HTMLDivElement>>;

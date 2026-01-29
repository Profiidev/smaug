import type BaseForm from 'positron-components/components/form/base-form.svelte';
import type { Component, ComponentProps, Snippet } from 'svelte';

export interface StageProps<T = undefined> {
  initialValue?: any;
  onsubmit: ComponentProps<typeof BaseForm>['onsubmit'];
  footer: Snippet<[{ isLoading: boolean }]>;
  isLoading: boolean;
  data: T;
}

export type StageComponent<T = undefined> = Component<
  StageProps<T>,
  { getValue: () => object | undefined }
>;

export interface Stage<T = undefined> {
  title: string;
  content: StageComponent<T>;
  data: object;
}

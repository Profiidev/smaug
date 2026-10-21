import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/svelte';
import { writable } from 'svelte/store';
import SectionErrorDot from '$lib/components/form/SectionErrorDot.svelte';

afterEach(cleanup);

const mountDot = (errors: Record<string, unknown>, keys: string[]) =>
  render(SectionErrorDot, {
    props: { formData: { errors: writable(errors) } as never, keys }
  });

describe('SectionErrorDot', () => {
  it('shows a dot when one of its keys has an error', () => {
    mountDot({ variables: { 0: { envVariable: ['Reserved'] } } }, [
      'variables'
    ]);
    expect(screen.getByLabelText('Has errors')).toBeInTheDocument();
  });

  it('stays hidden for errors on other keys', () => {
    mountDot({ name: ['Required'] }, ['variables']);
    expect(screen.queryByLabelText('Has errors')).not.toBeInTheDocument();
  });
});

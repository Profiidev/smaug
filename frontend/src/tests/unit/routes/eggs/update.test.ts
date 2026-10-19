import { describe, expect, it } from 'vitest';
import { type EggForm, formatData } from '$routes/eggs/schema.svelte';
import { changedSections, lineDiff, mergeSections } from '$routes/eggs/update';
import { eggInfo } from '$mocks/e2e/data';

const base = (): EggForm => formatData(structuredClone(eggInfo));

describe('changedSections', () => {
  it('is empty for identical eggs', () => {
    expect(changedSections(base(), base())).toEqual([]);
  });

  it('reports each section that differs', () => {
    const source = {
      ...base(),
      configStop: '^C',
      description: 'new',
      scriptsScript: 'echo new'
    };
    expect(changedSections(base(), source)).toEqual([
      'general',
      'install',
      'runtime'
    ]);
  });

  it('ignores the update URL', () => {
    expect(
      changedSections(base(), { ...base(), updateUrl: 'https://x.example' })
    ).toEqual([]);
  });
});

describe('mergeSections', () => {
  it('replaces only the chosen sections and keeps the update URL', () => {
    const current = { ...base(), updateUrl: 'https://mine.example' };
    const source = {
      ...base(),
      description: 'new',
      scriptsScript: 'echo new',
      updateUrl: ''
    };
    const merged = mergeSections(current, source, ['install']);
    expect(merged.scriptsScript).toBe('echo new');
    expect(merged.description).toBe(current.description);
    expect(mergeSections(current, source, ['general'])).toMatchObject({
      description: 'new',
      updateUrl: 'https://mine.example'
    });
  });
});

describe('lineDiff', () => {
  it('marks added, removed and unchanged lines in order', () => {
    expect(lineDiff('a\nb\nc', 'a\nx\nc\nd')).toEqual([
      { kind: 'ctx', text: 'a' },
      { kind: 'del', text: 'b' },
      { kind: 'add', text: 'x' },
      { kind: 'ctx', text: 'c' },
      { kind: 'add', text: 'd' }
    ]);
  });

  it('handles empty sides', () => {
    expect(lineDiff('', 'a')).toEqual([
      { kind: 'del', text: '' },
      { kind: 'add', text: 'a' }
    ]);
    expect(lineDiff('a', 'a')).toEqual([{ kind: 'ctx', text: 'a' }]);
  });
});

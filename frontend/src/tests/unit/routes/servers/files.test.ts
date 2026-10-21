import { describe, expect, it, vi } from 'vitest';
import type { FileEntry } from '$lib/client';
import {
  EDIT_LIMIT,
  changedLines,
  chunkRanges,
  encodePath,
  formatBytes,
  isEditable,
  joinPath,
  languageFor,
  normalize,
  relativeTime,
  resolvePath,
  segments,
  sendChunks,
  sortEntries
} from '$routes/servers/[uuid]/files';

const entry = (
  name: string,
  kind: FileEntry['kind'] = 'file',
  extra: Partial<FileEntry> = {}
): FileEntry => ({
  kind,
  mode: 0o644,
  modified: new Date(0),
  name,
  size: 10,
  ...extra
});

describe('isEditable', () => {
  it.each([
    'config.yml',
    'a.yaml',
    'ops.json',
    'server.properties',
    'readme.txt',
    'x.toml',
    'run.sh',
    'latest.log',
    'init.lua',
    'data.csv',
    '.env',
    'UPPER.YML'
  ])('edits %s', (name) => {
    expect(isEditable(entry(name))).toBe(true);
  });

  it.each(['plugin.jar', 'world.zip', 'level.dat', 'icon.png', 'noext'])(
    'downloads only %s',
    (name) => {
      expect(isEditable(entry(name))).toBe(false);
    }
  );

  it('accepts text mimes for unknown extensions', () => {
    expect(isEditable(entry('x.weird', 'file', { mime: 'text/plain' }))).toBe(
      true
    );
    expect(
      isEditable(entry('x.weird', 'file', { mime: 'application/json' }))
    ).toBe(true);
  });

  it('respects the 1 MiB limit', () => {
    expect(isEditable(entry('a.yml', 'file', { size: EDIT_LIMIT }))).toBe(true);
    expect(isEditable(entry('a.yml', 'file', { size: EDIT_LIMIT + 1 }))).toBe(
      false
    );
  });

  it('never edits folders or symlinks', () => {
    expect(isEditable(entry('conf.yml', 'dir'))).toBe(false);
    expect(isEditable(entry('conf.yml', 'symlink'))).toBe(false);
  });
});

describe('paths', () => {
  it.each([
    ['/', '/'],
    ['', '/'],
    ['//a///b/', '/a/b'],
    ['/a/./b', '/a/b'],
    ['/a/../b', '/b'],
    ['/../../x', '/x']
  ])('normalize(%j) = %j', (input, out) => {
    expect(normalize(input)).toBe(out);
  });

  it('joins', () => {
    expect(joinPath('/', 'a')).toBe('/a');
    expect(joinPath('/plugins', 'config.yml')).toBe('/plugins/config.yml');
  });

  it.each([
    ['/plugins', 'new.yml', '/plugins/new.yml'],
    ['/plugins', '../x', '/x'],
    ['/plugins/Ess', '../../x', '/x'],
    ['/plugins', '../../../x', '/x'],
    ['/plugins', 'sub/x', '/plugins/sub/x'],
    ['/plugins', '/abs/x', '/abs/x'],
    ['/plugins', './x', '/plugins/x']
  ])('resolvePath(%j, %j) = %j', (dir, input, out) => {
    expect(resolvePath(dir, input)).toBe(out);
  });

  it('splits breadcrumb segments', () => {
    expect(segments('/')).toEqual([]);
    expect(segments('/plugins/Essentials')).toEqual([
      { name: 'plugins', path: '/plugins' },
      { name: 'Essentials', path: '/plugins/Essentials' }
    ]);
  });

  it('keeps slashes readable in query values', () => {
    expect(encodePath('/a b/c&d')).toBe('/a%20b/c%26d');
  });
});

describe('sortEntries', () => {
  it('lists folders first, then by name (numeric aware)', () => {
    const sorted = sortEntries([
      entry('b.txt'),
      entry('zeta', 'dir'),
      entry('a10.txt'),
      entry('a2.txt'),
      entry('Alpha', 'dir')
    ]);
    expect(sorted.map((e) => e.name)).toEqual([
      'Alpha',
      'zeta',
      'a2.txt',
      'a10.txt',
      'b.txt'
    ]);
  });

  it('does not mutate the input', () => {
    const input = [entry('b'), entry('a')];
    sortEntries(input);
    expect(input.map((e) => e.name)).toEqual(['b', 'a']);
  });
});

describe('formatting', () => {
  it.each([
    [0, '0 B'],
    [78, '78 B'],
    [1024, '1.0 KiB'],
    [6451, '6.3 KiB'],
    [1.4 * 1024 * 1024, '1.4 MiB'],
    [5 * 1024 ** 4, '5.0 TiB']
  ])('formatBytes(%d) = %s', (bytes, out) => {
    expect(formatBytes(bytes)).toBe(out);
  });

  it('formats relative times', () => {
    const now = Date.parse('2026-01-10T12:00:00Z');
    expect(relativeTime(new Date(now - 10_000), now)).toBe('just now');
    expect(relativeTime(new Date(now - 3 * 3_600_000), now)).toBe(
      '3 hours ago'
    );
    expect(relativeTime(new Date(now - 2 * 86_400_000), now)).toBe(
      '2 days ago'
    );
    expect(relativeTime(new Date(now - 86_400_000), now)).toBe('yesterday');
  });
});

describe('editor helpers', () => {
  it.each([
    ['config.yml', 'yaml'],
    ['a.YAML', 'yaml'],
    ['ops.json', 'json'],
    ['server.properties', 'properties'],
    ['readme.txt', 'text'],
    ['noext', 'text']
  ])('languageFor(%s) = %s', (name, lang) => {
    expect(languageFor(name)).toBe(lang);
  });

  it('marks changed lines', () => {
    expect(changedLines('a\nb\nc', 'a\nb\nc')).toEqual([]);
    expect(changedLines('a\nb\nc', 'a\nB\nc')).toEqual([2]);
    expect(changedLines('a', 'a\nnew')).toEqual([2]);
    expect(changedLines('a\nb', 'a')).toEqual([]);
  });
});

describe('chunked upload', () => {
  it('splits into chunk ranges', () => {
    expect(chunkRanges(0, 4)).toEqual([]);
    expect(chunkRanges(4, 4)).toEqual([[0, 4]]);
    expect(chunkRanges(10, 4)).toEqual([
      [0, 4],
      [4, 8],
      [8, 10]
    ]);
  });

  it('hashes the whole file and sends every chunk', async () => {
    const sent: [number, string][] = [];
    const sha = await sendChunks(new Blob(['abc']), 1, async (i, chunk) => {
      sent.push([i, new TextDecoder().decode(chunk)]);
    });
    expect(sha).toBe(
      'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad'
    );
    expect(sent.toSorted((a, b) => a[0] - b[0])).toEqual([
      [0, 'a'],
      [1, 'b'],
      [2, 'c']
    ]);
  });

  it('hashes an empty file without sending', async () => {
    let calls = 0;
    const sha = await sendChunks(new Blob([]), 8, async () => {
      calls += 1;
    });
    expect(calls).toBe(0);
    expect(sha).toBe(
      'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'
    );
  });

  it('keeps at most `parallel` chunks in flight', async () => {
    let active = 0;
    let peak = 0;
    await sendChunks(new Blob(['x'.repeat(20)]), 2, async () => {
      active += 1;
      peak = Math.max(peak, active);
      await new Promise((r) => setTimeout(r, 5));
      active -= 1;
    });
    expect(peak).toBe(4);
  });

  it('rejects when a chunk fails', async () => {
    await expect(
      sendChunks(new Blob(['abcd']), 1, async (i) => {
        if (i === 1) {
          throw new Error('boom');
        }
      })
    ).rejects.toThrow('boom');
  });

  it('stops after the first failed chunk', async () => {
    const unhandled = vi.fn();
    process.on('unhandledRejection', unhandled);
    const sent: number[] = [];
    const boom = new Error('boom');
    try {
      await expect(
        sendChunks(
          new Blob(['abcdef']),
          1,
          async (i) => {
            sent.push(i);
            if (i === 1) {
              throw boom;
            }
          },
          1
        )
      ).rejects.toBe(boom);
      await new Promise((r) => setTimeout(r, 10));
      expect(sent).toEqual([0, 1]);
      expect(unhandled).not.toHaveBeenCalled();
    } finally {
      process.off('unhandledRejection', unhandled);
    }
  });
});

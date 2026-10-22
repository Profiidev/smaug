import { describe, expect, it } from 'vitest';
import { parseAnsi, stripAnsi } from '$routes/servers/[uuid]/ansi';

const ESC = '\u001B';

describe('parseAnsi', () => {
  it('keeps plain text as one segment and empty lines as none', () => {
    expect(parseAnsi('hello')).toEqual([{ text: 'hello' }]);
    expect(parseAnsi('')).toEqual([]);
  });

  it('maps basic and bright colours to theme variables', () => {
    expect(parseAnsi(`${ESC}[31mred${ESC}[92mgreen${ESC}[44mbg`)).toEqual([
      { fg: 'var(--ansi-1)', text: 'red' },
      { fg: 'var(--ansi-10)', text: 'green' },
      { bg: 'var(--ansi-4)', fg: 'var(--ansi-10)', text: 'bg' }
    ]);
    expect(parseAnsi(`${ESC}[103mx`)).toEqual([
      { bg: 'var(--ansi-11)', text: 'x' }
    ]);
  });

  it('handles 256-colour and truecolor', () => {
    expect(
      parseAnsi(`${ESC}[38;5;196ma${ESC}[38;5;9mb${ESC}[48;5;244mc`)
    ).toEqual([
      { fg: 'rgb(255 0 0)', text: 'a' },
      { fg: 'var(--ansi-9)', text: 'b' },
      { bg: 'rgb(128 128 128)', fg: 'var(--ansi-9)', text: 'c' }
    ]);
    expect(parseAnsi(`${ESC}[38;2;10;20;30;48;2;1;2;3mx`)).toEqual([
      { bg: 'rgb(1 2 3)', fg: 'rgb(10 20 30)', text: 'x' }
    ]);
  });

  it('resets mid-line and per attribute', () => {
    const segs = parseAnsi(
      `${ESC}[1;2;3;4;31ma${ESC}[22;23mb${ESC}[24;39mc${ESC}[0md`
    );
    expect(segs[0]).toEqual({
      bold: true,
      dim: true,
      fg: 'var(--ansi-1)',
      italic: true,
      text: 'a',
      underline: true
    });
    expect(segs[1]).toMatchObject({ fg: 'var(--ansi-1)', underline: true });
    expect(segs[1].bold).toBeUndefined();
    expect(segs[1].italic).toBeUndefined();
    expect(segs[2].fg).toBeUndefined();
    expect(segs[2].underline).toBeUndefined();
    expect(segs[3]).toEqual({ text: 'd' });
    expect(parseAnsi(`${ESC}[31ma${ESC}[mb`)[1]).toEqual({ text: 'b' });
  });

  it('parses Minecraft-style combined codes', () => {
    expect(parseAnsi(`${ESC}[0;36;1m[Server]${ESC}[0m Done`)).toEqual([
      { bold: true, fg: 'var(--ansi-6)', text: '[Server]' },
      { text: ' Done' }
    ]);
  });

  it('strips other escape sequences and carriage returns', () => {
    const line = `${ESC}[2K${ESC}[1Ga${ESC}]0;title\u0007b${ESC}]8;;x${ESC}\\c${ESC}(Bd\r`;
    expect(parseAnsi(line)).toEqual([
      { text: 'a' },
      { text: 'b' },
      { text: 'c' },
      { text: 'd' }
    ]);
    expect(stripAnsi(`${ESC}[31mWARN${ESC}[0m x${ESC}`)).toBe('WARN x');
  });
});

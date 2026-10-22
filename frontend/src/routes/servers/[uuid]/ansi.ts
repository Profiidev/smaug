export interface AnsiStyle {
  bg?: string;
  bold?: boolean;
  dim?: boolean;
  fg?: string;
  italic?: boolean;
  underline?: boolean;
}

export interface AnsiSegment extends AnsiStyle {
  text: string;
}

/** CSI (`params`, `final`), OSC (BEL or ST terminated), other ESC sequences, `\r`. */
const SEQUENCE =
  // oxlint-disable-next-line no-control-regex -- matching ESC/BEL is the point of an ANSI parser
  /\x1b\[(?<params>[0-?]*)[ -/]*(?<final>[@-~])?|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)?|\x1b[ -/]*[0-~]?|\r/gu;

const CUBE = [0, 95, 135, 175, 215, 255];

/** The 16 basic colours are theme variables (`--ansi-0`…`--ansi-15`), see ConsoleTab. */
const color256 = (n: number) => {
  if (n < 16) {
    return `var(--ansi-${n})`;
  }
  if (n < 232) {
    const i = n - 16;
    return `rgb(${CUBE[Math.floor(i / 36)]} ${CUBE[Math.floor(i / 6) % 6]} ${CUBE[i % 6]})`;
  }
  const g = 8 + (n - 232) * 10;
  return `rgb(${g} ${g} ${g})`;
};

const byte = (n: number | undefined) => Math.min(255, Math.max(0, n ?? 0));

/** Plain attribute set/reset codes. */
const ATTRS: Record<number, AnsiStyle> = {
  1: { bold: true },
  2: { dim: true },
  22: { bold: undefined, dim: undefined },
  23: { italic: undefined },
  24: { underline: undefined },
  3: { italic: true },
  39: { fg: undefined },
  4: { underline: true },
  49: { bg: undefined }
};

/** Basic and bright 16-colour codes, or undefined. */
const basicColor = (n: number): Partial<AnsiStyle> | undefined => {
  if (n >= 30 && n <= 37) {
    return { fg: color256(n - 30) };
  }
  if (n >= 90 && n <= 97) {
    return { fg: color256(n - 90 + 8) };
  }
  if (n >= 40 && n <= 47) {
    return { bg: color256(n - 40) };
  }
  if (n >= 100 && n <= 107) {
    return { bg: color256(n - 100 + 8) };
  }
  return undefined;
};

/** `38;5;n` / `38;2;r;g;b` (and 48 for bg) starting at `p[i]`: the colour and params consumed. */
const extendedColor = (
  p: number[],
  i: number
): [string | undefined, number] => {
  if (p[i + 1] === 5) {
    return [color256(byte(p[i + 2])), 2];
  }
  if (p[i + 1] === 2) {
    return [`rgb(${byte(p[i + 2])} ${byte(p[i + 3])} ${byte(p[i + 4])})`, 4];
  }
  return [undefined, 0];
};

/** Apply one SGR parameter list (`1;31`) to `style`, returning the new style. */
const applySgr = (style: AnsiStyle, params: string): AnsiStyle => {
  // Empty params (`ESC[m`, `ESC[;1m`) mean 0.
  const p = params.split(';').map((s) => Number(s) || 0);
  let next = { ...style };
  for (let i = 0; i < p.length; i += 1) {
    const n = p[i];
    if (n === 0) {
      next = {};
    } else if (n === 38 || n === 48) {
      const [color, used] = extendedColor(p, i);
      if (color) {
        next[n === 38 ? 'fg' : 'bg'] = color;
      }
      i += used;
    } else {
      next = { ...next, ...ATTRS[n], ...basicColor(n) };
    }
  }
  return next;
};

/** Split a log line into styled segments; non-SGR escapes are dropped. Style resets per line. */
export const parseAnsi = (line: string): AnsiSegment[] => {
  const segments: AnsiSegment[] = [];
  let style: AnsiStyle = {};
  let last = 0;
  const push = (text: string) => {
    if (text) {
      segments.push({ ...style, text });
    }
  };
  for (const m of line.matchAll(SEQUENCE)) {
    push(line.slice(last, m.index));
    last = m.index + m[0].length;
    if (m.groups?.final === 'm') {
      style = applySgr(style, m.groups.params);
    }
  }
  push(line.slice(last));
  return segments;
};

/** Line text without escape sequences. */
export const stripAnsi = (line: string) => line.replaceAll(SEQUENCE, '');

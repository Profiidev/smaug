interface Ctx {
  value: string;
  field: string;
  displayName: string;
  size: number;
  unit: string;
}

type Check = (param: string, ctx: Ctx) => string | undefined;

const bounds = (param: string) => param.split(',').map(Number);

const INTEGER = /^-?\d+$/;
const NUMERIC = /^-?\d+(?:\.\d+)?$/;

/** Emulates the `x` flag (JS has none): drops unescaped whitespace and `#` comments outside classes. */
const stripExtended = (pattern: string) =>
  pattern.replaceAll(/\\[\s\S]|\[(?:\\[\s\S]|[^\]])*\]|#[^\n]*|\s+/g, (m) =>
    m.startsWith('\\') || m.startsWith('[') ? m : ''
  );

/**
 * PCRE-style `<d>pattern<d>flags`: `d` is any character but alphanumerics,
 * whitespace and backslash; flags only `i`, `m`, `s`, `x`. Anything else
 * (malformed, unknown flag, invalid pattern) ignores the rule.
 */
const matchRegex = (param: string, value: string) => {
  const delim = param.charAt(0);
  const end = param.lastIndexOf(delim);
  const flags = param.slice(end + 1);
  if (
    !delim ||
    /[\p{L}\p{N}\s\\]/u.test(delim) ||
    end < 1 ||
    !/^[imsx]*$/.test(flags)
  ) {
    return true;
  }
  const pattern = param.slice(1, end);
  try {
    return new RegExp(
      flags.includes('x') ? stripExtended(pattern) : pattern,
      flags.replace('x', '')
    ).test(value);
  } catch {
    return true;
  }
};

const CHECKS: Record<string, Check> = {
  between: (param, { field, size, unit }) => {
    const [a, b] = bounds(param);
    return Number.isFinite(a) && Number.isFinite(b) && (size < a || size > b)
      ? `${field} must be between ${a} and ${b}${unit}.`
      : undefined;
  },
  boolean: (_, { field, value }) =>
    ['true', 'false', '1', '0'].includes(value)
      ? undefined
      : `${field} must be true or false.`,
  in: (param, { displayName, value }) =>
    param.split(',').includes(value)
      ? undefined
      : `The selected ${displayName} is invalid.`,
  integer: (_, { field, value }) =>
    INTEGER.test(value) ? undefined : `${field} must be an integer.`,
  max: (param, { field, size, unit }) => {
    const [a] = bounds(param);
    return Number.isFinite(a) && size > a
      ? `${field} must not be greater than ${a}${unit}.`
      : undefined;
  },
  min: (param, { field, size, unit }) => {
    const [a] = bounds(param);
    return Number.isFinite(a) && size < a
      ? `${field} must be at least ${a}${unit}.`
      : undefined;
  },
  numeric: (_, { field, value }) =>
    NUMERIC.test(value) ? undefined : `${field} must be a number.`,
  regex: (param, { field, value }) =>
    matchRegex(param, value) ? undefined : `${field} format is invalid.`
};

/**
 * Validates an egg variable value against its Laravel-style rules
 * (the subset the backend also enforces). Returns the first error or undefined.
 *
 * - Rules come as an array (`['required', 'max:20']`); elements containing `|`
 *   are split, except ones starting with `regex:` (use the array form for
 *   regexes with `|`). Unknown rules and malformed parameters are ignored.
 * - An empty (whitespace-only) value fails `required`; otherwise it skips
 *   every other rule.
 * - `integer`: `^-?\d+$`, `numeric`: `^-?\d+(\.\d+)?$`, `boolean`: one of
 *   `true`, `false`, `1`, `0`. `string` and `nullable` always pass.
 * - `min`/`max`/`between` compare the number when `integer` or `numeric` is
 *   present (skipped when the value is not numeric-shaped), else the length
 *   in code points.
 * - `in:a,b` needs an exact match, `regex:/pattern/flags` a JS RegExp match
 *   (any non-alphanumeric delimiter, e.g. `#pattern#i`).
 */
export const validateVariable = (
  value: string,
  rules: string[],
  displayName: string
): string | undefined => {
  const list = rules
    .flatMap((r) => (r.startsWith('regex:') ? [r] : r.split('|')))
    .map((r) => r.trim())
    .filter(Boolean);
  const field = `The ${displayName} field`;

  if (value.trim() === '') {
    return list.includes('required') ? `${field} is required.` : undefined;
  }

  const numeric = list.includes('integer') || list.includes('numeric');
  // NaN (not numeric-shaped) passes every size rule; the type rule reports it.
  const number = NUMERIC.test(value) ? Number(value) : Number.NaN;
  const ctx: Ctx = {
    displayName,
    field,
    // Code points, like Laravel's mb_strlen.
    // oxlint-disable-next-line no-misused-spread
    size: numeric ? number : [...value].length,
    unit: numeric ? '' : ' characters',
    value
  };

  for (const rule of list) {
    const sep = rule.indexOf(':');
    const name = sep === -1 ? rule : rule.slice(0, sep);
    const error = Object.hasOwn(CHECKS, name)
      ? CHECKS[name](sep === -1 ? '' : rule.slice(sep + 1), ctx)
      : undefined;
    if (error) {
      return error;
    }
  }
  return undefined;
};

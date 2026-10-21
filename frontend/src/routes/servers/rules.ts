interface Ctx {
  value: string;
  field: string;
  displayName: string;
  size: number;
  unit: string;
}

type Check = (param: string, ctx: Ctx) => string | undefined;

const bounds = (param: string) => param.split(',').map(Number);

const matchRegex = (param: string, value: string) => {
  const end = param.lastIndexOf('/');
  if (!param.startsWith('/') || end < 1) {
    return true;
  }
  try {
    return new RegExp(param.slice(1, end), param.slice(end + 1)).test(value);
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
    /^-?\d+$/.test(value) ? undefined : `${field} must be an integer.`,
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
    /^-?\d+(?:\.\d+)?$/.test(value) ? undefined : `${field} must be a number.`,
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
 * - An empty value fails `required`; otherwise it skips every other rule.
 * - `integer`: `^-?\d+$`, `numeric`: `^-?\d+(\.\d+)?$`, `boolean`: one of
 *   `true`, `false`, `1`, `0`. `string` and `nullable` always pass.
 * - `min`/`max`/`between` compare the number when `integer` or `numeric` is
 *   present, else the length in code points.
 * - `in:a,b` needs an exact match, `regex:/pattern/flags` a JS RegExp match.
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

  if (value === '') {
    return list.includes('required') ? `${field} is required.` : undefined;
  }

  const numeric = list.includes('integer') || list.includes('numeric');
  const ctx: Ctx = {
    displayName,
    field,
    // Code points, like Laravel's mb_strlen.
    // oxlint-disable-next-line no-misused-spread
    size: numeric ? Number(value) : [...value].length,
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

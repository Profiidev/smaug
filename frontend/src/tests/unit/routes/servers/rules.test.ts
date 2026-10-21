import { describe, expect, it } from 'vitest';
import { validateVariable } from '$routes/servers/rules';

const v = (value: string, rules: string[]) =>
  validateVariable(value, rules, 'Max players');

describe('validateVariable', () => {
  describe('required / nullable / empty values', () => {
    it('fails required on an empty value', () => {
      expect(v('', ['required'])).toBe('The Max players field is required.');
    });

    it('passes required on a non-empty value', () => {
      expect(v('x', ['required'])).toBeUndefined();
    });

    it('skips every other rule for an empty optional value', () => {
      expect(v('', ['nullable', 'integer', 'min:5'])).toBeUndefined();
      expect(v('', ['integer', 'regex:/^a$/'])).toBeUndefined();
    });

    it('passes with no rules', () => {
      expect(v('anything', [])).toBeUndefined();
    });
  });

  describe('rule parsing', () => {
    it('accepts a single pipe-joined element', () => {
      expect(v('', ['required|string|max:20'])).toBe(
        'The Max players field is required.'
      );
      expect(v('a'.repeat(21), ['required|string|max:20'])).toBe(
        'The Max players field must not be greater than 20 characters.'
      );
    });

    it('trims whitespace and drops empty segments', () => {
      expect(v('abc', [' max:2 ', '', '|'])).toBe(
        'The Max players field must not be greater than 2 characters.'
      );
    });

    it('does not split regex elements on pipes', () => {
      expect(v('b', ['regex:/^(a|b)$/'])).toBeUndefined();
      expect(v('c', ['regex:/^(a|b)$/'])).toBe(
        'The Max players field format is invalid.'
      );
    });

    it('ignores unknown rules and malformed parameters', () => {
      expect(
        v('x', ['alpha_dash', 'exists:users', 'max:abc', 'constructor'])
      ).toBeUndefined();
      expect(v('x', ['between:1'])).toBeUndefined();
    });

    it('returns the first failing rule in order', () => {
      expect(v('abc', ['max:1', 'integer'])).toBe(
        'The Max players field must be an integer.'
      );
      expect(v('5', ['integer', 'max:1', 'in:9'])).toBe(
        'The Max players field must not be greater than 1.'
      );
    });
  });

  describe('type rules', () => {
    it('always passes string', () => {
      expect(v('123', ['string'])).toBeUndefined();
    });

    it.each(['0', '42', '-7'])('accepts integer %s', (value) => {
      expect(v(value, ['integer'])).toBeUndefined();
    });

    it.each(['1.5', 'abc', '1e3', ' 1', '+1'])(
      'rejects non-integer %s',
      (value) => {
        expect(v(value, ['integer'])).toBe(
          'The Max players field must be an integer.'
        );
      }
    );

    it.each(['0', '-3', '2.5'])('accepts numeric %s', (value) => {
      expect(v(value, ['numeric'])).toBeUndefined();
    });

    it.each(['abc', '1.', '.5', '1,5'])('rejects non-numeric %s', (value) => {
      expect(v(value, ['numeric'])).toBe(
        'The Max players field must be a number.'
      );
    });

    it.each(['true', 'false', '1', '0'])('accepts boolean %s', (value) => {
      expect(v(value, ['boolean'])).toBeUndefined();
    });

    it.each(['yes', 'TRUE', '2'])('rejects non-boolean %s', (value) => {
      expect(v(value, ['boolean'])).toBe(
        'The Max players field must be true or false.'
      );
    });
  });

  describe('size rules', () => {
    it('compares the number when integer or numeric is present', () => {
      expect(v('10', ['integer', 'max:9'])).toBe(
        'The Max players field must not be greater than 9.'
      );
      expect(v('9', ['integer', 'max:9'])).toBeUndefined();
      expect(v('0.5', ['numeric', 'min:1'])).toBe(
        'The Max players field must be at least 1.'
      );
      expect(v('1', ['numeric', 'min:1'])).toBeUndefined();
    });

    it('compares the length otherwise', () => {
      expect(v('ab', ['min:3'])).toBe(
        'The Max players field must be at least 3 characters.'
      );
      expect(v('abc', ['min:3'])).toBeUndefined();
      expect(v('100', ['max:2'])).toBe(
        'The Max players field must not be greater than 2 characters.'
      );
    });

    it('counts code points, not UTF-16 units', () => {
      expect(v('😀😀', ['max:2'])).toBeUndefined();
    });

    it('checks between inclusively', () => {
      expect(v('1', ['integer', 'between:1,10'])).toBeUndefined();
      expect(v('10', ['integer', 'between:1,10'])).toBeUndefined();
      expect(v('11', ['integer', 'between:1,10'])).toBe(
        'The Max players field must be between 1 and 10.'
      );
      expect(v('a', ['between:2,4'])).toBe(
        'The Max players field must be between 2 and 4 characters.'
      );
    });
  });

  describe('in', () => {
    it('requires an exact listed value', () => {
      expect(v('normal', ['in:peaceful,normal,hard'])).toBeUndefined();
      expect(v('Normal', ['in:peaceful,normal,hard'])).toBe(
        'The selected Max players is invalid.'
      );
    });
  });

  describe('regex', () => {
    it('applies the pattern and flags', () => {
      expect(v('ABC', ['regex:/^[a-z]+$/i'])).toBeUndefined();
      expect(v('ABC', ['regex:/^[a-z]+$/'])).toBe(
        'The Max players field format is invalid.'
      );
    });

    it('keeps slashes inside the pattern', () => {
      expect(v('a/b', [String.raw`regex:/^a\/b$/`])).toBeUndefined();
      expect(v('a/b', ['regex:/^a/b$/'])).toBeUndefined();
    });

    it('ignores malformed or invalid regexes', () => {
      expect(v('x', ['regex:^a$'])).toBeUndefined();
      expect(v('x', ['regex:/'])).toBeUndefined();
      expect(v('x', ['regex:/(/'])).toBeUndefined();
      expect(v('x', ['regex:/a/x'])).toBeUndefined();
    });
  });
});

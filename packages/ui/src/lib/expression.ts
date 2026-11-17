/** Brings a number that is followed by a suffix, like the "cm" of "10cm", to the unit of the result */
export type Suffixes = Record<string, (value: number) => number>;

const NUMBER = /\s*((?:\d+\.?\d*|\.\d+)(?:e[-+]?\d+)?)/y;
const WORD = /\s*([a-z]+)/y;
const SYMBOL = /\s*([-+*/()])/y;

/**
 * Evaluates typed text like "2,5", "pi / 2" or "(1 + 2) * -3".
 * Returns undefined if the text is not an expression or has no finite result.
 */
export function evaluate(text: string, suffixes: Suffixes = {}): number | undefined {
  const source = text.toLowerCase().replaceAll(',', '.');
  let position = 0;

  function read(pattern: RegExp, accept?: (token: string) => boolean) {
    pattern.lastIndex = position;
    const token = pattern.exec(source)?.[1];
    if (token === undefined || (accept && !accept(token))) return undefined;
    position = pattern.lastIndex;
    return token;
  }

  const symbol = (symbols: string) => read(SYMBOL, (token) => symbols.includes(token));

  function sum(): number {
    let value = product();
    let operator: string | undefined;
    while ((operator = symbol('+-'))) {
      const right = product();
      value = operator === '+' ? value + right : value - right;
    }
    return value;
  }

  function product(): number {
    let value = operand();
    let operator: string | undefined;
    while ((operator = symbol('*/'))) {
      const right = operand();
      value = operator === '*' ? value * right : value / right;
    }
    return value;
  }

  function operand(): number {
    const sign = symbol('+-');
    if (sign) return sign === '-' ? -operand() : operand();

    if (symbol('(')) {
      const value = sum();
      return symbol(')') ? value : NaN;
    }

    const digits = read(NUMBER);
    if (digits !== undefined) {
      const suffix = read(WORD, (word) => Object.hasOwn(suffixes, word));
      return suffix ? suffixes[suffix](parseFloat(digits)) : parseFloat(digits);
    }

    return read(WORD, (word) => word === 'pi') ? Math.PI : NaN;
  }

  const result = sum();
  const complete = source.slice(position).trim() === '';
  return complete && Number.isFinite(result) ? result : undefined;
}

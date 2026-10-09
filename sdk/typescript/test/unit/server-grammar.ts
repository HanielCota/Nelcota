/**
 * A line-by-line port of the server's parsing of `in` lists and or/and
 * operands (crates/api/src/query/parse.rs: parse_in_list, split_top_level,
 * unquote_operand), so property tests can check the client's encoding
 * against the reading the server will do. The contract tests check the same
 * against the real binary.
 */

export function parseInList(value: string): string[] {
  if (!value.startsWith('(') || !value.endsWith(')')) throw new Error('not a list');
  const inner = value.slice(1, -1);
  if (inner.trim() === '') return [];
  const items: string[] = [];
  let current = '';
  let quoted = false;
  let wasQuoted = false;
  const chars = [...inner];
  for (let i = 0; i < chars.length; i++) {
    const c = chars[i]!;
    if (c === '"' && quoted) quoted = false;
    else if (c === '"' && current.trim() === '') {
      quoted = true;
      wasQuoted = true;
      current = '';
    } else if (c === '\\' && quoted) {
      const next = chars[++i];
      if (next !== undefined) current += next;
    } else if (c === ',' && !quoted) {
      items.push(wasQuoted ? current : current.trim());
      current = '';
      wasQuoted = false;
    } else current += c;
  }
  if (quoted) throw new Error('unclosed quotes');
  items.push(wasQuoted ? current : current.trim());
  return items;
}

export function splitTopLevel(input: string): string[] {
  const parts: string[] = [];
  let depth = 0;
  let quoted = false;
  let escaped = false;
  let start = 0;
  for (let i = 0; i < input.length; i++) {
    const c = input[i]!;
    if (quoted) {
      if (escaped) escaped = false;
      else if (c === '\\') escaped = true;
      else if (c === '"') quoted = false;
      continue;
    }
    if (c === '"') quoted = true;
    else if (c === '(') depth++;
    else if (c === ')') {
      if (depth === 0) throw new Error('unbalanced');
      depth--;
    } else if (c === ',' && depth === 0) {
      parts.push(input.slice(start, i));
      start = i + 1;
    }
  }
  if (quoted || depth !== 0) throw new Error('unbalanced');
  parts.push(input.slice(start));
  return parts;
}

/** `column.op."value"` → `[column, op, value]` as the server reads it. */
export function readFilter(item: string): [string, string, string] {
  const dot = item.indexOf('.');
  const column = item.slice(0, dot);
  const filter = item.slice(dot + 1);
  const opDot = filter.indexOf('.');
  const op = filter.slice(0, opDot);
  const operand = filter.slice(opDot + 1);
  if (op === 'in' || !operand.startsWith('"')) return [column, op, operand];
  if (!operand.endsWith('"') || operand.length < 2) throw new Error('unclosed quotes');
  let value = '';
  const chars = [...operand.slice(1, -1)];
  for (let i = 0; i < chars.length; i++) {
    if (chars[i] === '\\') {
      const next = chars[++i];
      if (next !== undefined) value += next;
    } else value += chars[i];
  }
  return [column, op, value];
}

/** Pretty-print valid JSON without converting its number or string tokens. */
export function formatJson(value: string): string {
  JSON.parse(value) // Validate syntax; the parsed values are never serialized.
  const tokens = value.match(/"(?:[^"\\]|\\[\s\S])*"|-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?|true|false|null|[{}\[\],:]/g) ?? []
  let depth = 0
  let result = ''
  const newline = () => { result += '\n' + '  '.repeat(depth) }
  for (let i = 0; i < tokens.length; i++) {
    const token = tokens[i]
    if (token === '{' || token === '[') {
      result += token
      depth++
      if (tokens[i + 1] !== '}' && tokens[i + 1] !== ']') newline()
    } else if (token === '}' || token === ']') {
      depth--
      if (tokens[i - 1] !== '{' && tokens[i - 1] !== '[') newline()
      result += token
    } else if (token === ',') {
      result += token
      newline()
    } else {
      result += token === ':' ? ': ' : token
    }
  }
  return result
}

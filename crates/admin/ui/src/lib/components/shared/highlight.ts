// A small highlighter for the panel's read-only code blocks: TypeScript and
// JavaScript snippets, shell commands, SQL fragments and .env lines. It only
// colours (strings, comments, keywords, calls, types, numbers) and never
// changes the text, so copying still gives the exact code. The colours are the
// SQL editor's --syntax-* tokens (D92). An editor-grade parser would mean a new
// dependency per language for a few short snippets.

export type CodeLang = 'ts' | 'sh' | 'sql' | 'env'
export type TokenKind = 'keyword' | 'function' | 'type' | 'string' | 'number' | 'comment'
export interface Token {
  text: string
  kind?: TokenKind
}

const TS_KEYWORDS = new Set(
  'import export from as type interface const let var function return await async new if else for of in while do switch case break continue throw try catch finally class extends implements typeof keyof satisfies default void true false null undefined this'.split(' '),
)
const SQL_KEYWORDS = new Set(
  'select from where and or not in is null true false exists as on join left right inner outer using with check insert into update set delete values returning order by group having limit offset case when then else end distinct any all create alter drop table policy for to grant revoke execute function public'.split(' '),
)
const SH_COMMANDS = new Set('curl npm npx pnpm yarn bun deno node nelcota export'.split(' '))

const STRING = /'(?:\\.|[^'\\\n])*'?|"(?:\\.|[^"\\\n])*"?/y
const TEMPLATE = /`(?:\\.|[^`\\])*`?/y
const NUMBER = /\d+(?:\.\d+)?(?![\w.])/y
const WORD = /[A-Za-z_$][\w$]*/y
const FLAG = /--?[A-Za-z][\w-]*/y

function at(re: RegExp, code: string, i: number): string | null {
  re.lastIndex = i
  return re.exec(code)?.[0] ?? null
}

const lineEnd = (code: string, i: number) => {
  const end = code.indexOf('\n', i)
  return end < 0 ? code.length : end
}

/** `code` as coloured tokens; joining every `text` gives `code` back. */
export function highlight(code: string, lang: CodeLang): Token[] {
  if (lang === 'env') return env(code)
  const out: Token[] = []
  const push = (text: string, kind?: TokenKind) => {
    const last = out[out.length - 1]
    if (!kind && last && !last.kind) last.text += text
    else out.push(kind ? { text, kind } : { text })
  }
  // A shell word is a command at the start of a line or after a pipe.
  let commandSlot = true
  let i = 0
  while (i < code.length) {
    const ch = code[i]
    const prev = i > 0 ? code[i - 1] : '\n'
    const wordStart = /\s/.test(prev)
    let text: string | null = null
    let kind: TokenKind | undefined
    if (lang === 'ts' && code.startsWith('//', i) && prev !== ':') [text, kind] = [code.slice(i, lineEnd(code, i)), 'comment']
    else if (lang === 'ts' && code.startsWith('/*', i)) {
      const end = code.indexOf('*/', i + 2)
      ;[text, kind] = [code.slice(i, end < 0 ? code.length : end + 2), 'comment']
    } else if (lang === 'sh' && ch === '#' && wordStart) [text, kind] = [code.slice(i, lineEnd(code, i)), 'comment']
    else if (lang === 'sql' && code.startsWith('--', i)) [text, kind] = [code.slice(i, lineEnd(code, i)), 'comment']
    else if ((text = at(STRING, code, i) ?? (lang === 'ts' ? at(TEMPLATE, code, i) : null))) kind = 'string'
    else if (lang === 'sh' && ch === '-' && wordStart && (text = at(FLAG, code, i))) kind = 'keyword'
    else if (/\d/.test(ch) && !/[\w.$]/.test(prev) && (text = at(NUMBER, code, i))) kind = 'number'
    else if (!/[\w$]/.test(prev) && (text = at(WORD, code, i))) kind = word(lang, text, prev, code.slice(i + text.length).match(/^\s*(.)/)?.[1], commandSlot)
    if (text) {
      commandSlot = false
    } else {
      text = ch
      if (ch === '\n' || ch === '|') commandSlot = true
    }
    push(text, kind)
    i += text.length
  }
  return out
}

function word(lang: CodeLang, text: string, prev: string, next: string | undefined, commandSlot: boolean): TokenKind | undefined {
  if (lang === 'sh') return commandSlot && SH_COMMANDS.has(text) ? 'function' : undefined
  if (lang === 'sql') return SQL_KEYWORDS.has(text.toLowerCase()) ? 'keyword' : next === '(' ? 'function' : undefined
  if (prev !== '.' && TS_KEYWORDS.has(text)) return 'keyword'
  if (next === '(' || (next === '<' && /^[a-z]/.test(text))) return 'function'
  if (/^[A-Z]/.test(text)) return 'type'
  return undefined
}

/** .env lines: the name, then its value; `#` lines are comments. */
function env(code: string): Token[] {
  const out: Token[] = []
  for (const line of code.split(/(\n)/)) {
    const m = line.match(/^(\s*)([A-Za-z_]\w*)(=)(.*)$/)
    if (line.trimStart().startsWith('#')) out.push({ text: line, kind: 'comment' })
    else if (m) {
      if (m[1]) out.push({ text: m[1] })
      out.push({ text: m[2], kind: 'function' }, { text: m[3] })
      if (m[4]) out.push({ text: m[4], kind: 'string' })
    } else if (line) out.push({ text: line })
  }
  return out
}

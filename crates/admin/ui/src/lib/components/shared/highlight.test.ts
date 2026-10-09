import { describe, expect, it } from 'vitest'
import { highlight, type CodeLang } from './highlight'

const kinds = (code: string, lang: CodeLang) => highlight(code, lang).filter((t) => t.kind).map((t) => `${t.kind}:${t.text}`)
const roundTrip = (code: string, lang: CodeLang) => highlight(code, lang).map((t) => t.text).join('')

describe('highlight', () => {
  const client = "import { createClient } from '@nelcota/client'\nimport type { Database } from './database'\n\nexport const nelcota = createClient<Database>('http://127.0.0.1:8000')"

  it('never changes the text', () => {
    for (const [code, lang] of [
      [client, 'ts'],
      ["curl -X POST 'http://x/rest/v1/todos' \\\n  -H 'Content-Type: application/json' # note", 'sh'],
      ['(owner = auth.uid()) AND exists (select 1)', 'sql'],
      ['NELCOTA_SMTP_URL=\n# comment\nNELCOTA_SIGNUP_ENABLED=true', 'env'],
      ["unterminated 'string", 'ts'],
    ] as const) expect(roundTrip(code, lang)).toBe(code)
  })

  it('colours a TypeScript snippet', () => {
    expect(kinds(client, 'ts')).toEqual([
      'keyword:import', 'keyword:from', "string:'@nelcota/client'",
      'keyword:import', 'keyword:type', 'type:Database', 'keyword:from', "string:'./database'",
      'keyword:export', 'keyword:const', 'function:createClient', 'type:Database', "string:'http://127.0.0.1:8000'",
    ])
  })

  it('keeps URLs inside strings and comments apart', () => {
    expect(kinds("fetch('http://a') // done", 'ts')).toEqual(['function:fetch', "string:'http://a'", 'comment:// done'])
  })

  it('colours shell commands and flags, not every word', () => {
    expect(kinds("npm install @nelcota/client\ncurl -H 'a: b' x | nelcota types", 'sh')).toEqual([
      'function:npm', 'function:curl', 'keyword:-H', "string:'a: b'", 'function:nelcota',
    ])
  })

  it('colours SQL keywords in any case and calls', () => {
    expect(kinds('(owner = auth.uid()) AND Exists', 'sql')).toEqual(['function:uid', 'keyword:AND', 'keyword:Exists'])
  })

  it('splits .env lines into name and value', () => {
    expect(kinds('A_B=\nC=true', 'env')).toEqual(['function:A_B', 'function:C', 'string:true'])
  })
})

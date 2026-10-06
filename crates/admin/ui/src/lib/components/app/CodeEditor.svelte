<script lang="ts">
  import { onMount } from 'svelte'
  import { basicSetup } from 'codemirror'
  import { EditorView, keymap } from '@codemirror/view'
  import { Compartment, EditorState, Prec } from '@codemirror/state'
  import { indentWithTab } from '@codemirror/commands'
  import { sql, PostgreSQL } from '@codemirror/lang-sql'
  import { HighlightStyle, syntaxHighlighting } from '@codemirror/language'
  import { tags as t } from '@lezer/highlight'

  let {
    value = $bindable(''),
    schema = {},
    defaultSchema = 'public',
    onrun,
  }: {
    value?: string
    /** `{ "tabela": ["col", ...], "auth.users": [...] }` para o autocomplete. */
    schema?: Record<string, string[]>
    defaultSchema?: string
    onrun: () => void
  } = $props()

  let host: HTMLDivElement
  let view: EditorView | undefined
  const language = new Compartment()

  // Cores do tema vêm das variáveis CSS do painel: acompanham claro/escuro.
  const theme = EditorView.theme({
    '&': { height: '100%', fontSize: '13px', backgroundColor: 'transparent', color: 'var(--foreground)' },
    '.cm-scroller': { fontFamily: 'var(--font-mono)', lineHeight: '1.65' },
    '.cm-content': { padding: '14px 0', caretColor: 'var(--primary)' },
    '.cm-gutters': { backgroundColor: 'transparent', color: 'var(--muted-foreground)', border: 'none' },
    '.cm-lineNumbers .cm-gutterElement': { padding: '0 12px 0 16px', opacity: '0.6' },
    '.cm-activeLine, .cm-activeLineGutter': { backgroundColor: 'color-mix(in oklch, var(--foreground) 4%, transparent)' },
    '.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--primary)', borderLeftWidth: '2px' },
    '&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection': {
      backgroundColor: 'color-mix(in oklch, var(--primary) 22%, transparent) !important',
    },
    '&.cm-focused': { outline: 'none' },
    '.cm-matchingBracket': { backgroundColor: 'color-mix(in oklch, var(--primary) 18%, transparent)', outline: 'none' },
    '.cm-tooltip': {
      backgroundColor: 'var(--popover)',
      color: 'var(--popover-foreground)',
      border: '1px solid var(--border)',
      borderRadius: '8px',
      overflow: 'hidden',
      boxShadow: '0 10px 30px rgb(0 0 0 / 0.25)',
    },
    '.cm-tooltip-autocomplete > ul': { fontFamily: 'var(--font-mono)', fontSize: '12px', maxHeight: '16em' },
    '.cm-tooltip-autocomplete > ul > li': { padding: '3px 10px !important' },
    '.cm-tooltip-autocomplete > ul > li[aria-selected]': {
      backgroundColor: 'color-mix(in oklch, var(--primary) 18%, transparent)',
      color: 'var(--foreground)',
    },
    '.cm-completionDetail': { color: 'var(--muted-foreground)', fontStyle: 'normal', marginLeft: '8px' },
    '.cm-foldPlaceholder': { backgroundColor: 'var(--muted)', border: 'none', color: 'var(--muted-foreground)' },
  })

  const highlight = HighlightStyle.define([
    { tag: [t.keyword, t.operatorKeyword, t.modifier], color: 'oklch(0.75 0.15 160)', fontWeight: '500' },
    { tag: [t.string, t.special(t.string)], color: 'oklch(0.8 0.13 75)' },
    { tag: [t.number, t.bool, t.null], color: 'oklch(0.75 0.14 300)' },
    { tag: [t.comment, t.lineComment, t.blockComment], color: 'var(--muted-foreground)', fontStyle: 'italic' },
    { tag: [t.typeName, t.standard(t.name)], color: 'oklch(0.75 0.12 230)' },
    { tag: [t.function(t.variableName), t.function(t.name)], color: 'oklch(0.78 0.12 200)' },
    { tag: [t.operator, t.punctuation, t.bracket], color: 'var(--muted-foreground)' },
    { tag: [t.special(t.name)], color: 'var(--foreground)' },
  ])

  const sqlExtension = () =>
    sql({ dialect: PostgreSQL, schema, defaultSchema, upperCaseKeywords: false })

  onMount(() => {
    view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: value,
        extensions: [
          basicSetup,
          Prec.highest(
            keymap.of([
              {
                key: 'Mod-Enter',
                run: () => {
                  onrun()
                  return true
                },
              },
            ]),
          ),
          keymap.of([indentWithTab]),
          language.of(sqlExtension()),
          syntaxHighlighting(highlight),
          theme,
          EditorView.lineWrapping,
          EditorView.updateListener.of((update) => {
            if (update.docChanged) value = update.state.doc.toString()
          }),
        ],
      }),
    })
    view.focus()
    return () => view?.destroy()
  })

  // Schema carregado depois (autocomplete) ou texto trocado por fora (histórico).
  $effect(() => {
    void schema
    view?.dispatch({ effects: language.reconfigure(sqlExtension()) })
  })
  $effect(() => {
    if (view && value !== view.state.doc.toString()) {
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: value } })
    }
  })
</script>

<div bind:this={host} class="h-full min-h-0 overflow-hidden"></div>

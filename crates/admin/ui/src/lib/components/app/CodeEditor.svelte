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
    '&': { height: '100%', fontSize: '14px', backgroundColor: 'transparent', color: 'var(--foreground)' },
    '.cm-scroller': { fontFamily: 'var(--font-mono)', lineHeight: '1.75' },
    '.cm-content': { padding: '12px 0', caretColor: 'var(--foreground)' },
    '.cm-line': { padding: '0 16px 0 8px' },
    '.cm-gutters': { backgroundColor: 'transparent', color: 'var(--muted-foreground)', border: 'none' },
    '.cm-lineNumbers .cm-gutterElement': { padding: '0 8px 0 16px', minWidth: '3em', opacity: '0.6' },
    '.cm-lineNumbers .cm-gutterElement.cm-activeLineGutter': { color: 'var(--foreground)', opacity: '1' },
    '.cm-activeLine, .cm-activeLineGutter': { backgroundColor: 'color-mix(in oklch, var(--foreground) 4%, transparent)' },
    '.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--foreground)' },
    '&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection': {
      backgroundColor: 'color-mix(in oklch, var(--foreground) 18%, transparent) !important',
    },
    '&.cm-focused': { outline: 'none' },
    '.cm-matchingBracket': { backgroundColor: 'color-mix(in oklch, var(--foreground) 15%, transparent)', outline: 'none' },
    '.cm-tooltip': {
      backgroundColor: 'var(--popover)',
      color: 'var(--popover-foreground)',
      border: '1px solid var(--border)',
      borderRadius: '6px',
      boxShadow: 'var(--elev-overlay)',
      overflow: 'hidden',
    },
    '.cm-tooltip-autocomplete > ul': { fontFamily: 'var(--font-mono)', fontSize: '13px', maxHeight: '18em', padding: '4px' },
    '.cm-tooltip-autocomplete > ul > li': { padding: '5px 10px !important', borderRadius: '6px' },
    '.cm-tooltip-autocomplete > ul > li[aria-selected]': {
      backgroundColor: 'var(--accent)',
      color: 'var(--foreground)',
    },
    '.cm-completionDetail': { color: 'var(--muted-foreground)', fontStyle: 'normal', marginLeft: '8px' },
    '.cm-foldPlaceholder': { backgroundColor: 'var(--muted)', border: 'none', color: 'var(--muted-foreground)' },
  })

  const highlight = HighlightStyle.define([
    // Paleta contida: palavras-chave em destaque, literais e comentários atenuados.
    { tag: [t.keyword, t.operatorKeyword, t.modifier], color: 'var(--foreground)', fontWeight: '500' },
    { tag: [t.string, t.special(t.string)], color: 'oklch(0.72 0.09 150)' },
    { tag: [t.number, t.bool, t.null], color: 'oklch(0.72 0.08 60)' },
    { tag: [t.comment, t.lineComment, t.blockComment], color: 'var(--muted-foreground)' },
    { tag: [t.typeName, t.standard(t.name)], color: 'var(--foreground)' },
    { tag: [t.function(t.variableName), t.function(t.name)], color: 'var(--foreground)' },
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
          EditorView.contentAttributes.of({ 'aria-label': 'Editor SQL' }),
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

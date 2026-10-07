<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Label } from '$lib/components/ui/label'
  import ShieldCheck from '@lucide/svelte/icons/shield-check'
  import ShieldAlert from '@lucide/svelte/icons/shield-alert'
  import GrantsEditor from './GrantsEditor.svelte'
  import ConfirmDialog from './ConfirmDialog.svelte'
  import { API_ROLES, grantChanges, type AlterAction, type GrantDef, type Structure } from '$lib/ddl'

  let {
    structure,
    onalter,
    ondrop,
  }: {
    structure: Structure
    /** Aplica as ações; resolve depois de recarregar a estrutura. */
    onalter: (actions: AlterAction[]) => Promise<void>
    ondrop: () => void
  } = $props()

  const fromStructure = (s: Structure): GrantDef[] =>
    API_ROLES.map((role) => ({ role, privileges: [...(s.grants.find((g) => g.role === role)?.privileges ?? [])] }))

  let name = $state('')
  let comment = $state('')
  let grants = $state<GrantDef[]>([])
  let disableRlsOpen = $state(false)

  // Rascunhos voltam ao estado salvo sempre que a estrutura recarrega.
  $effect(() => {
    name = structure.name
    comment = structure.comment ?? ''
    grants = fromStructure(structure)
  })

  const identityChanges = $derived.by((): AlterAction[] => {
    const actions: AlterAction[] = []
    if ((comment.trim() || null) !== structure.comment) actions.push({ action: 'set_comment', comment: comment.trim() || null })
    if (name.trim() && name.trim() !== structure.name) actions.push({ action: 'rename_table', name: name.trim() })
    return actions
  })
  const pendingGrants = $derived(grantChanges(structure.grants, grants))
</script>

<div class="grid gap-6">
  <section class="rounded-lg border bg-card p-4">
    <h2 class="text-sm font-medium">Tabela</h2>
    <form
      class="mt-3 grid gap-3 sm:grid-cols-[1fr_2fr_auto] sm:items-end"
      onsubmit={(e) => {
        e.preventDefault()
        onalter(identityChanges)
      }}
    >
      <div class="grid gap-1.5">
        <Label for="settings-name" class="font-normal text-muted-foreground">Nome</Label>
        <Input id="settings-name" bind:value={name} class="font-mono" />
      </div>
      <div class="grid gap-1.5">
        <Label for="settings-comment" class="font-normal text-muted-foreground">Descrição</Label>
        <Input id="settings-comment" bind:value={comment} placeholder="aparece na documentação da API" />
      </div>
      <Button type="submit" variant="outline" disabled={identityChanges.length === 0}>Salvar</Button>
    </form>
  </section>

  <section class="flex flex-wrap items-center gap-4 rounded-lg border bg-card p-4">
    {#if structure.rls_enabled}
      <ShieldCheck class="size-5 text-brand" />
      <div class="flex-1">
        <h2 class="text-sm font-medium">Row Level Security ativo</h2>
        <p class="text-xs font-light text-muted-foreground">As policies decidem quais linhas cada role enxerga.</p>
      </div>
      <Button variant="outline" size="sm" onclick={() => (disableRlsOpen = true)}>Desativar</Button>
    {:else}
      <ShieldAlert class="size-5 text-destructive" />
      <div class="flex-1">
        <h2 class="text-sm font-medium text-destructive">Row Level Security desligado</h2>
        <p class="text-xs font-light text-muted-foreground">Quem tem GRANT lê e altera todas as linhas.</p>
      </div>
      <Button size="sm" onclick={() => onalter([{ action: 'set_rls', enabled: true }])}>Ativar RLS</Button>
    {/if}
  </section>

  <section class="grid gap-3 rounded-lg border bg-card p-4">
    <div class="flex items-center justify-between gap-3">
      <div>
        <h2 class="text-sm font-medium">Acesso pela API (GRANT)</h2>
        <p class="text-xs font-light text-muted-foreground">Quais operações cada role pode tentar; o RLS filtra as linhas.</p>
      </div>
      <Button variant="outline" size="sm" disabled={pendingGrants.length === 0} onclick={() => onalter(pendingGrants)}>
        Salvar GRANTs
      </Button>
    </div>
    <GrantsEditor bind:grants />
  </section>

  <section class="flex flex-wrap items-center gap-4 rounded-lg border border-destructive/30 p-4">
    <div class="flex-1">
      <h2 class="text-sm font-medium">Apagar tabela</h2>
      <p class="text-xs font-light text-muted-foreground">Remove a tabela, as linhas e as policies. Não dá para desfazer.</p>
    </div>
    <Button variant="destructive" size="sm" onclick={ondrop}>Apagar tabela</Button>
  </section>
</div>

<ConfirmDialog
  bind:open={disableRlsOpen}
  title="Desativar o RLS?"
  description="Sem RLS, as policies deixam de valer: quem tem GRANT na tabela lê e altera todas as linhas."
  confirmLabel="Desativar"
  destructive
  onconfirm={() => onalter([{ action: 'set_rls', enabled: false }])}
/>

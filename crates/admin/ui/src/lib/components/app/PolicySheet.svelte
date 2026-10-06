<script lang="ts">
  import * as Sheet from '$lib/components/ui/sheet'
  import * as Select from '$lib/components/ui/select'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Label } from '$lib/components/ui/label'
  import { Checkbox } from '$lib/components/ui/checkbox'
  import { Textarea } from '$lib/components/ui/textarea'
  import Sparkles from '@lucide/svelte/icons/sparkles'
  import { toast } from 'svelte-sonner'
  import SqlPreview from './SqlPreview.svelte'
  import { ddl, policyFields, type ApiRole, type PolicyCommand, type PolicyDef } from '$lib/ddl'
  import { POLICY_TEMPLATES, guessOwnerColumn, type PolicyTemplate } from '$lib/policy-templates'
  import { SqlPreview as Preview } from '$lib/preview.svelte'

  let {
    open = $bindable(false),
    table,
    original = null,
    onsaved,
  }: {
    open?: boolean
    table: string
    /** Policy sendo editada; `null` = nova. */
    original?: PolicyDef | null
    onsaved: () => void
  } = $props()

  const COMMANDS: { value: PolicyCommand; label: string }[] = [
    { value: 'select', label: 'SELECT (ler)' },
    { value: 'insert', label: 'INSERT (criar)' },
    { value: 'update', label: 'UPDATE (alterar)' },
    { value: 'delete', label: 'DELETE (apagar)' },
    { value: 'all', label: 'ALL (tudo)' },
  ]
  const ROLES: { value: ApiRole; hint: string }[] = [
    { value: 'anon', hint: 'sem login' },
    { value: 'authenticated', hint: 'logados' },
    { value: 'service_role', hint: 'backend' },
  ]

  const blank = (): PolicyDef => ({
    name: '',
    command: 'select',
    roles: ['authenticated'],
    permissive: true,
    using: '',
    check: '',
  })

  let policy = $state<PolicyDef>(blank())
  let ownerColumn = $state('user_id')
  let saving = $state(false)
  const preview = new Preview()
  const fields = $derived(policyFields(policy.command))

  $effect(() => {
    if (!open) return
    policy = original ? { ...original, roles: [...original.roles] } : blank()
    // A coluna do dono (uuid) alimenta os modelos.
    ddl
      .structure(table)
      .then((s) => (ownerColumn = guessOwnerColumn(s.columns)))
      .catch(() => (ownerColumn = 'user_id'))
  })

  /** Só manda as expressões que o comando aceita. */
  const payload = $derived<PolicyDef>({
    ...policy,
    name: policy.name.trim(),
    using: fields.using ? policy.using?.trim() || null : null,
    check: fields.check ? policy.check?.trim() || null : null,
  })
  const ready = $derived(payload.name !== '' && (payload.command === 'insert' ? !!payload.check : !!payload.using))

  $effect(() => {
    const snapshot = $state.snapshot(payload)
    if (!open || !ready) return preview.clear()
    preview.schedule((signal) =>
      original
        ? ddl.replacePolicy(table, original.name, snapshot, true, { signal })
        : ddl.createPolicy(table, snapshot, true, { signal }),
    )
  })

  function applyTemplate(template: PolicyTemplate) {
    const built = template.build(ownerColumn)
    policy = { ...built, permissive: true, using: built.using ?? '', check: built.check ?? '' }
  }

  function toggleRole(role: ApiRole, on: boolean) {
    policy.roles = on ? [...policy.roles, role] : policy.roles.filter((r) => r !== role)
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault()
    saving = true
    try {
      const snapshot = $state.snapshot(payload)
      const result = original
        ? await ddl.replacePolicy(table, original.name, snapshot)
        : await ddl.createPolicy(table, snapshot)
      toast.success(result.message ?? 'Policy salva')
      open = false
      onsaved()
    } catch (e) {
      toast.error((e as Error).message)
    } finally {
      saving = false
    }
  }

  const commandLabel = (value: PolicyCommand) => COMMANDS.find((c) => c.value === value)?.label ?? value
</script>

<Sheet.Root bind:open>
  <Sheet.Content class="flex w-full flex-col gap-0 p-0 data-[side=right]:sm:max-w-2xl">
    <Sheet.Header class="border-b px-6 py-4">
      <Sheet.Title>{original ? `Editar policy` : 'Nova policy'}</Sheet.Title>
      <Sheet.Description>em <code class="font-mono text-foreground">{table}</code></Sheet.Description>
    </Sheet.Header>

    <form id="policy-form" class="flex-1 space-y-5 overflow-y-auto px-6 py-5" onsubmit={submit}>
      <DropdownMenu.Root>
        <DropdownMenu.Trigger>
          {#snippet child({ props })}
            <Button variant="outline" size="sm" {...props}><Sparkles />Começar de um modelo</Button>
          {/snippet}
        </DropdownMenu.Trigger>
        <DropdownMenu.Content align="start" class="w-80">
          <DropdownMenu.Label class="text-xs font-normal text-muted-foreground">
            Modelos de dono usam a coluna <code class="text-foreground">{ownerColumn}</code>
          </DropdownMenu.Label>
          {#each POLICY_TEMPLATES as template (template.label)}
            <DropdownMenu.Item onclick={() => applyTemplate(template)} class="flex-col items-start gap-0">
              <span>{template.label}</span>
              <span class="text-xs font-light text-muted-foreground">{template.description}</span>
            </DropdownMenu.Item>
          {/each}
        </DropdownMenu.Content>
      </DropdownMenu.Root>

      <div class="grid gap-4 sm:grid-cols-2">
        <div class="grid gap-1.5">
          <Label for="policy-name" class="font-normal text-muted-foreground">Nome</Label>
          <Input id="policy-name" bind:value={policy.name} placeholder="ex.: dono lê as próprias notas" required />
        </div>
        <div class="grid gap-1.5">
          <Label class="font-normal text-muted-foreground">Comando</Label>
          <Select.Root type="single" bind:value={policy.command}>
            <Select.Trigger class="w-full">{commandLabel(policy.command)}</Select.Trigger>
            <Select.Content>
              {#each COMMANDS as command (command.value)}
                <Select.Item value={command.value}>{command.label}</Select.Item>
              {/each}
            </Select.Content>
          </Select.Root>
        </div>
      </div>

      <fieldset class="grid gap-2">
        <legend class="mb-1 text-sm text-muted-foreground">Vale para</legend>
        <div class="flex flex-wrap gap-4">
          {#each ROLES as role (role.value)}
            <label class="flex items-center gap-2 text-sm">
              <Checkbox checked={policy.roles.includes(role.value)} onCheckedChange={(v) => toggleRole(role.value, v === true)} />
              <span class="font-mono text-xs">{role.value}</span>
              <span class="text-xs font-light text-muted-foreground">{role.hint}</span>
            </label>
          {/each}
        </div>
        {#if policy.roles.length === 0}
          <p class="text-xs font-light text-muted-foreground">Nenhuma marcada: vale para todas as roles (PUBLIC).</p>
        {/if}
      </fieldset>

      <label class="flex items-start gap-3 text-sm">
        <Checkbox checked={!policy.permissive} onCheckedChange={(v) => (policy.permissive = v !== true)} class="mt-0.5" />
        <span>
          Restritiva
          <span class="block text-xs font-light text-muted-foreground">
            Permissivas somam acesso (basta uma liberar). Restritivas são exigidas além delas.
          </span>
        </span>
      </label>

      {#if fields.using}
        <div class="grid gap-1.5">
          <Label for="policy-using" class="font-normal text-muted-foreground">
            USING <span class="font-light">· quais linhas existentes a role {policy.command === 'delete' ? 'pode apagar' : 'enxerga'}</span>
          </Label>
          <Textarea
            id="policy-using"
            bind:value={() => policy.using ?? '', (v) => (policy.using = v)}
            placeholder={`${ownerColumn} = auth.uid()`}
            class="min-h-20 font-mono text-xs"
          />
        </div>
      {/if}
      {#if fields.check}
        <div class="grid gap-1.5">
          <Label for="policy-check" class="font-normal text-muted-foreground">
            WITH CHECK <span class="font-light">· quais linhas novas ou alteradas são aceitas</span>
          </Label>
          <Textarea
            id="policy-check"
            bind:value={() => policy.check ?? '', (v) => (policy.check = v)}
            placeholder={policy.command === 'insert' ? `${ownerColumn} = auth.uid()` : 'vazio = mesma regra do USING'}
            class="min-h-20 font-mono text-xs"
          />
        </div>
      {/if}
      <p class="text-xs font-light text-muted-foreground">
        Use <code class="text-foreground">auth.uid()</code> para o id do usuário logado e
        <code class="text-foreground">auth.jwt()</code> para as claims do token.
      </p>

      <SqlPreview {preview} placeholder="Dê um nome e escreva a expressão para ver o SQL." />
    </form>

    <Sheet.Footer class="flex-row justify-end gap-2 border-t px-6 py-4">
      <Button variant="outline" onclick={() => (open = false)}>Cancelar</Button>
      <Button type="submit" form="policy-form" disabled={saving || !ready}>
        {saving ? 'Salvando…' : original ? 'Salvar policy' : 'Criar policy'}
      </Button>
    </Sheet.Footer>
  </Sheet.Content>
</Sheet.Root>

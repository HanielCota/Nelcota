from pathlib import Path

root = Path('D:/Nelcota/crates/admin/ui/src')
def edit(name, replacements):
    path = root / name
    content = path.read_text(encoding='utf-8')
    for before, after in replacements:
        assert before in content, (name, before)
        content = content.replace(before, after)
    path.write_text(content, encoding='utf-8', newline='\n')

edit('lib/features/api/ApiPage.svelte', [
    ('<!-- The visible section stays identifiable in the sticky tab line. -->', '<!-- Keep the active section visible while scrolling through the cards. -->'),
    ('sticky top-0 z-10 -my-3 bg-background py-3', 'sticky top-0 z-10 -my-3 bg-background/90 py-3 backdrop-blur'),
    ("current === section.id ? 'border-foreground font-medium text-foreground' : 'border-transparent text-muted-foreground hover:border-border-strong hover:text-foreground'", "current === section.id ? 'bg-nav-active font-medium text-nav-active-foreground' : 'text-muted-foreground hover:text-foreground'"),
    ('grid content-start gap-1.5 border-l pl-4', 'grid content-start gap-1.5 rounded-2xl bg-well p-4'),
    ('flex flex-col gap-3 border-l pl-4', 'flex flex-col gap-3 rounded-2xl bg-well p-4'),
    ('<Icon class="size-5 shrink-0 text-muted-foreground" aria-hidden="true" />', '<span class="grid size-10 shrink-0 place-items-center rounded-full bg-card text-muted-foreground"><Icon class="size-[18px]" aria-hidden="true" /></span>'),
    ('w-52 max-w-full rounded-lg font-mono', 'w-52 max-w-full rounded-full font-mono'),
    ('flex h-10 items-center gap-1 rounded-full bg-well p-1 sm:ml-auto', 'flex max-w-full flex-wrap items-center gap-1 rounded-xl bg-well p-1 sm:ml-auto sm:rounded-full'),
])
edit('lib/features/sign-in/SignIn.svelte', [
    ('grid gap-x-8 sm:grid-cols-2', 'grid gap-4 sm:grid-cols-2 xl:grid-cols-3'),
    ('grid gap-1 border-b px-1 py-4', 'grid gap-1 rounded-3xl bg-card p-5'),
    ('block rounded-xl bg-well px-3 py-2 text-xs break-all', 'block rounded-full bg-well px-3 py-1 text-xs break-all'),
])
for name in ['lib/features/policies/Policies.svelte', 'lib/features/migrations/Migrations.svelte']:
    edit(name, [('grid gap-1 border-b px-1 py-4', 'grid gap-1 rounded-3xl bg-card p-5')])
edit('lib/features/policies/Policies.svelte', [
    ('<Rows3 class="size-5 shrink-0 text-muted-foreground" aria-hidden="true" />', '<span class="grid size-10 shrink-0 place-items-center rounded-full bg-well text-muted-foreground"><Rows3 class="size-[18px]" aria-hidden="true" /></span>'),
])
edit('lib/features/migrations/Migrations.svelte', [
    ('rounded-lg bg-well', 'rounded-2xl bg-well'),
])
for name in ['lib/features/storage/Storage.svelte', 'lib/features/users/Users.svelte']:
    edit(name, [('rounded-xl border bg-card', 'rounded-3xl bg-card')])
edit('lib/shell/components/TopNav.svelte', [('max-w-page px-4 py-3 sm:px-6', 'max-w-page px-4 py-4 sm:px-6')])
edit('lib/shell/AuthenticatedShell.svelte', [('flex h-dvh flex-col gap-4 overflow-hidden bg-background sm:gap-6', 'flex h-dvh flex-col overflow-hidden bg-background')])
edit('lib/features/auth/Login.svelte', [
    ("  import { Badge } from '$lib/components/ui/badge'\n", ''),
    ('relative flex min-h-screen', 'relative flex min-h-dvh'),
    ('relative w-full max-w-[420px] pt-[8.6rem]', 'relative w-full max-w-[400px]'),
    ('pointer-events-none absolute top-0 left-1/2 z-10 size-36', 'pointer-events-none absolute -top-[8.6rem] left-1/2 size-36'),
    ('flex flex-col gap-6 rounded-xl border border-border bg-card px-6 py-8 sm:px-8', 'flex flex-col gap-5 rounded-3xl bg-card px-6 pt-10 pb-6 sm:px-8 sm:pb-8'),
    ('      <div class="grid gap-2">', '      <div class="text-center">'),
    ('''        <div class="flex h-7 max-w-full items-center justify-center">
          {#if project}
            <Badge variant="outline" class="h-7 max-w-full gap-1.5 px-3">
              <span>{t('login.projectLabel')}</span>
              <span class="min-w-0 truncate" translate="no" title={project}>{project}</span>
            </Badge>
          {:else}
            <p class="text-sm text-muted-foreground">{t('login.adminPanel')}</p>
          {/if}
        </div>''', '''        <p class="mt-1 text-sm text-muted-foreground [overflow-wrap:anywhere]">
          {#if project}{t('login.projectPanel')} <span class="font-medium text-foreground" translate="no">{project}</span>{:else}{t('login.adminPanel')}{/if}
        </p>'''),
    ('<Field.Group>', '<Field.Group class="gap-5">'),
    ('class="h-12"', 'class="h-10"'),
    ('size="icon"', 'size="icon-sm"'),
    ('type="submit" size="lg" class="w-full"', 'type="submit" class="mt-1 h-10 w-full"'),
])
print('Restored remaining visual sections; responsive and form behavior preserved.')

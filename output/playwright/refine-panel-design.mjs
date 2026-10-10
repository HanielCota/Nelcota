import { readFileSync, writeFileSync, readdirSync } from 'node:fs'
import { join } from 'node:path'

const root = 'D:/Nelcota/crates/admin/ui/src'
const changed = []
function edit(relative, transform) {
  const file = join(root, relative)
  const before = readFileSync(file, 'utf8')
  const after = transform(before)
  if (after !== before) { writeFileSync(file, after); changed.push(relative) }
}
function walk(dir) {
  for (const item of readdirSync(join(root, dir), { withFileTypes: true })) {
    const relative = `${dir}/${item.name}`
    if (item.isDirectory()) walk(relative)
    else if (item.name.endsWith('.svelte')) edit(relative, text => text
      .replaceAll('rounded-3xl bg-card', 'rounded-xl border bg-card')
      .replaceAll('rounded-3xl', 'rounded-xl')
      .replaceAll('rounded-2xl', 'rounded-lg'))
  }
}
for (const dir of ['lib/features', 'lib/components/shared', 'lib/shell']) walk(dir)

edit('app.css', text => text.replace('--radius: 0.75rem;', '--radius: 0.625rem;'))
edit('lib/components/ui/button/button.svelte', text => text.replace('rounded-full border border-transparent', 'rounded-lg border border-transparent'))
edit('lib/components/ui/input/input.svelte', text => text.replaceAll(' [&[type=search]]:rounded-full', ''))
edit('lib/components/ui/dialog/dialog-content.svelte', text => text.replace('rounded-3xl p-6', 'rounded-xl p-6'))
edit('lib/components/ui/card/card.svelte', text => text.replace('overflow-hidden rounded-2xl', 'overflow-hidden rounded-xl border'))

for (const relative of ['lib/features/tables/components/TableSidebar.svelte', 'lib/features/sql/components/SqlSidebar.svelte', 'lib/features/tables/components/TableToolbar.svelte', 'lib/features/policies/Policies.svelte', 'lib/features/api/ApiPage.svelte']) {
  edit(relative, text => text.replaceAll('rounded-full', 'rounded-lg'))
}
for (const relative of ['lib/features/policies/Policies.svelte', 'lib/features/migrations/Migrations.svelte', 'lib/features/sign-in/SignIn.svelte']) {
  edit(relative, text => text
    .replaceAll('grid gap-1 rounded-xl border bg-card p-5', 'grid gap-1 border-b px-1 py-4')
    .replaceAll('text-3xl font-semibold tracking-tight tabular-nums', 'text-2xl font-semibold tabular-nums')
    .replaceAll('font-mono text-3xl font-semibold tracking-tight tabular-nums', 'font-mono text-2xl font-semibold tabular-nums'))
}
edit('lib/features/auth/Login.svelte', text => text
  .replaceAll('rounded-full bg-card', 'rounded-lg bg-card')
  .replace('h-8 cursor-pointer rounded-full', 'h-8 cursor-pointer rounded-md')
  .replace(' bg-card px-6 py-8 shadow-raised', ' bg-card px-6 py-8')
  .replace(/    <div aria-hidden="true" class="pointer-events-none absolute top-\[8\.3rem\].*?<\/div>\r?\n/, ''))
console.log(JSON.stringify({ changed }, null, 2))

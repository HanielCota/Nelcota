from pathlib import Path
from datetime import datetime
import re
import shutil
import subprocess

ROOT = Path('D:/Nelcota')
UI = 'crates/admin/ui/src/'
BACKUP = ROOT / 'output' / ('layout-before-restore-' + datetime.now().strftime('%Y%m%d-%H%M%S'))
modified = subprocess.check_output(['git', 'diff', '--name-only', '--', UI], cwd=ROOT, text=True).splitlines()
for name in modified:
    destination = BACKUP / name
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT / name, destination)

def original(name):
    return subprocess.check_output(['git', 'show', 'HEAD:' + UI + name], cwd=ROOT).decode('utf-8')

def write(name, content):
    (ROOT / UI / name).write_text(content, encoding='utf-8', newline='\n')

# These files only changed appearance. Keep the edited versions in the backup.
pure = [
    'lib/components/ui/button/button.svelte', 'lib/components/ui/input/input.svelte',
    'lib/components/ui/textarea/textarea.svelte', 'lib/components/ui/card/card.svelte',
    'lib/components/ui/select/select-trigger.svelte', 'lib/components/ui/dialog/dialog-content.svelte',
    'lib/components/shared/Callout.svelte', 'lib/components/shared/EmptyState.svelte',
    'lib/components/shared/LoadError.svelte', 'lib/components/shared/PillTabs.svelte',
    'lib/features/api/components/ServiceTokenCard.svelte',
    'lib/features/policies/components/RecentlyBlocked.svelte',
    'lib/features/profile/components/Avatar.svelte', 'lib/shell/components/AccountMenu.svelte',
    'lib/features/sign-in/components/OptionCard.svelte', 'lib/shell/NotFound.svelte',
    'lib/features/tables/TableEditor.svelte', 'lib/features/tables/components/ColumnFields.svelte',
    'lib/features/tables/components/ColumnSheet.svelte', 'lib/features/tables/components/DataGrid.svelte',
    'lib/features/tables/components/RowSheet.svelte', 'lib/features/tables/components/StructureColumns.svelte',
    'lib/features/tables/components/StructureView.svelte', 'lib/features/tables/components/TableSettings.svelte',
]
for name in pure:
    write(name, original(name))

# Restore presentation on matching markup, leaving changed behavior and responsive fixes intact.
quoted = re.compile(r'''(["'])([^"'\n]+)\1''')
def is_style(value):
    return any(token in value for token in ['rounded-', 'bg-card', 'bg-well', 'border-t', 'border-b', 'font-semibold', 'shrink-0', 'items-center', 'gap-', 'text-muted-foreground']) and (' ' in value or value.startswith('rounded-'))
def normalized(line):
    return quoted.sub(lambda match: match.group(1) + '__STYLE__' + match.group(1) if is_style(match.group(2)) else match.group(0), line)

skip = set(pure + ['lib/features/overview/Overview.svelte', 'lib/features/projects/Projects.svelte',
    'lib/features/overview/components/TrafficChart.svelte', 'lib/features/tables/components/CreateTableSheet.svelte',
    'lib/features/policies/components/PolicySheet.svelte', 'lib/components/shared/PageHeader.svelte',
    'lib/shell/AuthenticatedShell.svelte', 'lib/shell/components/Logo.svelte',
    'lib/features/projects/components/ProjectSwitcher.svelte',
    'lib/features/sql/SqlEditor.svelte', 'lib/features/sql/components/SqlSidebar.svelte'])
for full_name in modified:
    name = full_name.removeprefix(UI)
    if name in skip or not name.endswith('.svelte'):
        continue
    old_lines = {}
    for line in original(name).splitlines():
        old_lines.setdefault(normalized(line), set()).add(line)
    current = (ROOT / full_name).read_text(encoding='utf-8')
    result = []
    for line in current.splitlines():
        candidates = old_lines.get(normalized(line), set())
        if len(candidates) == 1:
            old = next(iter(candidates))
            # Only presentation differences, never behavior, labels or bindings.
            if old != line and quoted.sub(lambda m: '__STYLE__' if is_style(m.group(2)) else m.group(0), old) == quoted.sub(lambda m: '__STYLE__' if is_style(m.group(2)) else m.group(0), line):
                line = old
        result.append(line)
    write(name, '\n'.join(result) + '\n')

overview = original('lib/features/overview/Overview.svelte')
overview = overview.replace("  import { Skeleton } from '$lib/components/ui/skeleton'", "  import { Skeleton } from '$lib/components/ui/skeleton'\n  import { Button } from '$lib/components/ui/button'\n  import Plus from '@lucide/svelte/icons/plus'\n  import UserPlus from '@lucide/svelte/icons/user-plus'\n  import Plug from '@lucide/svelte/icons/plug'")
overview = overview.replace('  <!-- Headline: the page\'s name and the period\'s traffic. -->', '''  <nav class="flex flex-wrap justify-end gap-2" aria-label={t('overview.quickActions')}>
    <Button size="sm" href={href('/tables?create=true')}><Plus data-icon="inline-start" aria-hidden="true" />{t('tables.editor.newTable')}</Button>
    <Button variant="outline" size="sm" href={href('/users?create=true')}><UserPlus data-icon="inline-start" aria-hidden="true" />{t('users.new')}</Button>
    <Button variant="outline" size="sm" href={href('/connect')}><Plug data-icon="inline-start" aria-hidden="true" />{t('connect.start')}</Button>
  </nav>
  <!-- Headline: the page's name and the period's traffic. -->''')
write('lib/features/overview/Overview.svelte', overview)

chart = original('lib/features/overview/components/TrafficChart.svelte').replace('class="block touch-none', 'class="block max-w-full touch-none')
write('lib/features/overview/components/TrafficChart.svelte', chart)

projects = original('lib/features/projects/Projects.svelte')
projects = projects.replace("  import { api } from '$lib/api'", "  import { api } from '$lib/api'\n  import { href } from '$lib/router.svelte'")
projects = projects.replace('          {#if !project.current && project.url}', '''          {#if project.current}
            <Button variant="outline" class="mt-auto w-full" href={href('/')}>{t('projects.back')}</Button>
          {:else if project.url}''')
projects = projects.replace("{t('projects.open')}<ArrowUpRight />", "{t('projects.open')}<ArrowUpRight data-icon=\"inline-end\" aria-hidden=\"true\" />")
write('lib/features/projects/Projects.svelte', projects)

css = (ROOT / UI / 'app.css').read_text(encoding='utf-8')
css = css.replace('\t/* Focus stays neutral; green is reserved for actions and status. */\n\t--ring: oklch(0.52 0 0);', '\t--ring: var(--brand);')
css = css.replace('--radius: 0.625rem;', '--radius: 0.75rem;').replace('--ring: oklch(0.74 0 0);', '--ring: oklch(0.87 0.16 152);')
css = css.replace('outline: 2px solid var(--ring);', 'outline: 2px solid var(--brand);')
write('app.css', css)
print(f'Visual restored. Recoverable source backup: {BACKUP}')

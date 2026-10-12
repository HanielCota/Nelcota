import { expect, test } from '@playwright/test'
import { fixture } from './fixtures'

test('edits enum arrays as complete literals with nullable elements', async ({ page }) => {
  await fixture(page)
  const original = await page.evaluate(async () => (await fetch('/admin/api/tables/notes')).json())
  original.table.columns.push({
    name: 'states', type: 'public.order_status[]', full_type: 'public.order_status[]',
    category: 'A', nullable: false, has_default: false, generated: false,
    enum_values: ['open', 'paid'], is_pk: false, comment: null, references: null,
  })
  for (const row of original.rows) row.states = '{open,NULL}'
  await page.route('**/admin/api/tables/notes?*', route => route.fulfill({ json: original }))
  let patch: { pk: Record<string, string>; values: Record<string, string | null> } | undefined
  await page.route('**/admin/api/tables/notes/rows', async route => {
    patch = route.request().postDataJSON()
    await route.fulfill({ json: { count: 1 } })
  })

  await page.goto('/admin/tables/notes')
  await page.getByRole('button', { name: 'Expandir linha 1', exact: true }).click()
  const field = page.locator('textarea#f-states')
  await expect(field).toHaveValue('{open,NULL}')
  await field.fill('{paid,NULL,open}')
  await page.getByRole('button', { name: 'Salvar', exact: true }).click()
  await expect(page.getByRole('dialog')).toBeHidden()
  expect(patch).toEqual({ pk: { id: original.rows[0].id }, values: { states: '{paid,NULL,open}' } })
})

import { test, expect } from '@playwright/test'
import { fixture } from './fixtures'
import { editing } from './editing'
import { draftsAndFiles } from './draftsAndFiles'
import { uploadsAndDetails } from './uploadsAndDetails'

test.beforeEach(async ({ page }) => { await fixture(page) })
for (const route of ['policies', 'migrations', 'storage', 'projects', 'connect']) {
  test(`${route} loads its generated contract without an error`, async ({ page }) => {
    await page.goto(`/admin/${route}`)
    await expect(page.getByRole('heading', { level: 1 })).toBeVisible()
    await page.waitForLoadState('networkidle')
    await expect(page.getByRole('alert')).toHaveCount(0)
  })
}
test('editing retains row identity, supports keyboard NULL and protects dirty drafts', async ({ page }) => {
  await page.goto('/admin/tables/notes')
  await page.locator('[data-cell="0:1"]').waitFor()
  await editing(page)
})
test('SQL drafts survive navigation and refresh, files preview and uploads complete', async ({ page }) => {
  await page.goto('/admin/sql')
  await page.getByRole('textbox', { name: 'Editor SQL', exact: true }).waitFor()
  await draftsAndFiles(page)
})
test('upload conflicts keep their original destination and SQL cells show full content', async ({ page }) => { await uploadsAndDetails(page) })
test('an incompatible API response can be retried without losing the page', async ({ page }) => {
  let failed = true
  await page.route('**/admin/api/users?*', async route => {
    if (failed) await route.fulfill({ json: { users: 'incompatible' } })
    else await route.fulfill({ json: { page: 0, users: [], total: 0, has_next: false } })
  })
  await page.goto('/admin/users')
  await expect(page.getByRole('alert')).toContainText('resposta incompatível')
  failed = false
  await page.getByRole('button', { name: 'Tentar de novo', exact: true }).click()
  await expect(page.getByRole('alert')).toHaveCount(0)
})

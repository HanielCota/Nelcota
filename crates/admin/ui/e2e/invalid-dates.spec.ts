import { expect, test } from '@playwright/test'
import { fixture } from './fixtures'

test('an invalid user timestamp remains readable without breaking the page', async ({ page }) => {
  const errors: string[] = []
  page.on('pageerror', error => errors.push(error.message))
  await fixture(page)
  const users = await page.evaluate(async () => (await fetch('/admin/api/users')).json())
  users.users[0].created_at = 'not-a-date'
  users.users[0].last_sign_in_at = null
  await page.route('**/admin/api/users?*', route => route.fulfill({ json: users }))
  await page.goto('/admin/users')
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Usuários')
  await expect(page.getByRole('cell', { name: 'not-a-date', exact: true })).toBeVisible()
  expect(errors).toEqual([])
})

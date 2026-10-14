import { test, expect } from '@playwright/test'
import { fixture } from './fixtures'

for (const action of ['Cancelar', 'Substituir']) {
  test(`closing an upload conflict with ${action} releases the original File`, async ({ page, context }) => {
    await fixture(page)
    await page.route('**/admin/api/storage/buckets/documents/upload?*', route => {
      const replacing = new URL(route.request().url()).searchParams.get('replace') === 'true'
      if (replacing) return route.fulfill({ json: {} })
      return route.fulfill({ status: 409, json: { error: 'Already exists', code: 'object_exists' } })
    })
    await page.goto('/admin/storage/documents')
    const profiler = await context.newCDPSession(page)
    try {
      for (let round = 0; round < 3; round++) {
        const files = await page.evaluateHandle(() => {
          const data = new DataTransfer()
          data.items.add(new File([new Uint8Array(1024 * 1024)], 'existing.txt', { type: 'text/plain' }))
          Reflect.set(window, 'conflictingFile', new WeakRef(data.files[0]!))
          return data
        })
        await page.getByRole('button', { name: 'Arraste arquivos para cá ou clique para selecionar', exact: true }).dispatchEvent('drop', { dataTransfer: files })
        // A Playwright handle would itself retain the File and hide cleanup.
        await files.dispose()
        await expect(page.getByRole('alertdialog')).toBeVisible()
        expect(await page.evaluate(() => Reflect.get(window, 'conflictingFile').deref() !== undefined)).toBe(true)
        await page.getByRole('alertdialog').getByRole('button', { name: action, exact: true }).click()
        await expect(page.getByRole('alertdialog')).toHaveCount(0)
        await expect.poll(async () => {
          await profiler.send('HeapProfiler.collectGarbage')
          return page.evaluate(() => Reflect.get(window, 'conflictingFile').deref() !== undefined)
        }).toBe(false)
      }
    } finally {
      await profiler.detach()
    }
  })
}

import { test, expect, type Page } from '@playwright/test'
const id = 'a'.repeat(64),
  digest = 'b'.repeat(64)
const base = {
  email: 'operator@example.test',
  host_management: true,
  projects: [],
  jobs: [],
  release: null,
  enabled: false,
  checking: false,
  check_error: null
}
const project = {
  name: 'shop',
  domain: 'api.shop.test',
  version: '0.4.0',
  app: 'healthy',
  database: 'healthy',
  backups: ['nelcota-shop-pair.dump']
}
const host = {
  receipt: null as object | null,
  enabled: true,
  projects: [project],
  jobs: [] as object[],
  max_projects: 32,
  ram_mb: 8192,
  free_bytes: 10000000000
}
async function mount(page: Page) {
  await page.route('**/admin/', async (route) => {
    const response = await route.fetch()
    await route.fulfill({
      response,
      body: (await response.text()).replace(
        '<head>',
        '<head><meta name="nelcota-updates-controller" content="1">'
      )
    })
  })
}
function plan(
  operation: object,
  confirmation: string,
  blockers: string[] = []
) {
  return {
    id,
    operation,
    confirmation,
    blockers,
    expires_at: Math.floor(Date.now() / 1000) + 300,
    version: '0.4.0',
    image_digest: digest,
    project_count: 1,
    ram_mb: 8192,
    free_bytes: 10000000000,
    destructive:
      confirmation.startsWith('RESTORE') || confirmation.startsWith('REMOVE')
  }
}
async function confirm(page: Page, phrase: string) {
  await page
    .getByLabel('Conferi o projeto, o domínio e o impacto desta operação.')
    .check()
  await page
    .getByLabel('Tenho acesso SSH e compreendo como recuperar a VPS.')
    .check()
  await page.getByLabel('Digite exatamente a confirmação abaixo').fill(phrase)
  await page
    .getByLabel('Confirme a senha do operador')
    .fill('operator-test-password')
}
test('creation requires visual review, both acknowledgements, exact phrase and password; lost replies retain one operation', async ({
  page
}, info) => {
  await mount(page)
  let starts = 0,
    accepted: Record<string, unknown> | null = null
  await page.route('**/admin/api/updates', async (route) => {
    if (route.request().method() === 'GET') {
      await route.fulfill({ json: { result: 'status', data: base } })
      return
    }
    const body = route.request().postDataJSON()
    expect(route.request().headers()['x-nelcota-updates']).toBe('1')
    if (body.action === 'host_status') {
      await route.fulfill({
        json: {
          result: 'host_status',
          data: {
            ...host,
            receipt: body.key ? accepted : null,
            jobs: body.key ? [] : accepted ? [accepted] : []
          }
        }
      })
      return
    }
    if (body.action === 'host_plan') {
      await route.fulfill({
        json: {
          result: 'host_plan',
          data: plan(body.operation, 'CREATE reporting')
        }
      })
      return
    }
    if (body.action === 'host_start') {
      starts++
      expect(body.confirmation).toBe('CREATE reporting')
      accepted = {
        id: digest,
        request_key: body.key,
        operation: {
          kind: 'create',
          project: 'reporting',
          domain: 'api.reporting.test',
          profile: '1gb'
        },
        stage: 'starting',
        sequence: 4,
        updated_at: 123,
        changed: true,
        backup: null,
        error: null,
        last_stage: null
      }
      await route.abort('failed')
      return
    }
    throw new Error(`Unexpected action: ${body.action}`)
  })
  await page.goto('/admin/')
  await page.getByLabel('Nome do projeto').fill('reporting')
  await page.getByLabel('Domínio da API').fill('api.reporting.test')
  await page
    .getByRole('button', { name: 'Validar e revisar', exact: true })
    .click()
  await expect(
    page.getByRole('heading', { name: 'Revise antes de confirmar' })
  ).toBeVisible()
  const execute = page.getByRole('button', {
    name: 'Confirmar e executar',
    exact: true
  })
  await expect(execute).toBeDisabled()
  await page
    .getByLabel('Confirme a senha do operador')
    .fill('operator-test-password')
  await expect(execute).toBeDisabled()
  await confirm(page, 'CREATE reporting')
  await expect(execute).toBeEnabled()
  await page.screenshot({
    path: info.outputPath('creation-review.png'),
    fullPage: true
  })
  await execute.click()
  await expect(
    page.getByRole('status').filter({ hasText: 'Iniciando serviços' })
  ).toBeVisible()
  await page.reload()
  await expect(
    page.getByRole('status').filter({ hasText: 'Iniciando serviços' })
  ).toBeVisible()
  expect(starts).toBe(1)
  expect(
    await page.evaluate(() =>
      sessionStorage.getItem('nelcota.host.request:operator@example.test')
    )
  ).toBeNull()
})
test('DNS blockers remain visible and prohibit execution', async ({ page }) => {
  await mount(page)
  await page.route('**/admin/api/updates', async (route) => {
    if (route.request().method() === 'GET') {
      await route.fulfill({ json: { result: 'status', data: base } })
      return
    }
    const body = route.request().postDataJSON()
    if (body.action === 'host_status') {
      await route.fulfill({ json: { result: 'host_status', data: host } })
      return
    }
    if (body.action === 'host_plan') {
      await route.fulfill({
        json: {
          result: 'host_plan',
          data: plan(body.operation, 'CREATE reporting', ['dns_mismatch'])
        }
      })
      return
    }
    throw new Error('A blocked operation was submitted')
  })
  await page.goto('/admin/')
  await page.getByLabel('Nome do projeto').fill('reporting')
  await page.getByLabel('Domínio da API').fill('api.reporting.test')
  await page
    .getByRole('button', { name: 'Validar e revisar', exact: true })
    .click()
  await expect(
    page.getByText(
      'O DNS não aponta exclusivamente para os endereços aprovados da VPS.'
    )
  ).toBeVisible()
  await expect(
    page.getByRole('button', { name: 'Confirmar e executar', exact: true })
  ).toBeDisabled()
  await expect(page.getByLabel('Confirme a senha do operador')).toBeDisabled()
})
test('restore review identifies the exact backup and warns about replacing data', async ({
  page
}, info) => {
  await mount(page)
  await page.route('**/admin/api/updates', async (route) => {
    if (route.request().method() === 'GET') {
      await route.fulfill({ json: { result: 'status', data: base } })
      return
    }
    const body = route.request().postDataJSON()
    if (body.action === 'host_status') {
      await route.fulfill({ json: { result: 'host_status', data: host } })
      return
    }
    if (body.action === 'host_plan') {
      expect(body.operation).toEqual({
        kind: 'restore',
        project: 'shop',
        backup: 'nelcota-shop-pair.dump'
      })
      await route.fulfill({
        json: {
          result: 'host_plan',
          data: plan(body.operation, 'RESTORE shop')
        }
      })
      return
    }
    throw new Error('An unconfirmed restore was submitted')
  })
  await page.goto('/admin/')
  await page
    .getByRole('button', { name: 'Selecionar projeto', exact: true })
    .click()
  await page
    .getByLabel('Backup selecionado')
    .selectOption('nelcota-shop-pair.dump')
  await page
    .getByRole('button', { name: 'Restaurar backup', exact: true })
    .click()
  await expect(
    page.getByText('nelcota-shop-pair.dump', { exact: true }).last()
  ).toBeVisible()
  await expect(
    page.getByText(
      /Os dados e arquivos serão substituídos pelo par selecionado/
    )
  ).toBeVisible()
  await expect(
    page.getByRole('button', { name: 'Confirmar e executar', exact: true })
  ).toBeDisabled()
  await page.screenshot({
    path: info.outputPath('restore-review.png'),
    fullPage: true
  })
})

test('removal names the target and explains the verified private archive before approval', async ({
  page
}, info) => {
  await mount(page)
  await page.route('**/admin/api/updates', async (route) => {
    if (route.request().method() === 'GET') {
      await route.fulfill({ json: { result: 'status', data: base } })
      return
    }
    const body = route.request().postDataJSON()
    if (body.action === 'host_status') {
      await route.fulfill({ json: { result: 'host_status', data: host } })
      return
    }
    if (body.action === 'host_plan') {
      expect(body.operation).toEqual({ kind: 'remove', project: 'shop' })
      await route.fulfill({
        json: { result: 'host_plan', data: plan(body.operation, 'REMOVE shop') }
      })
      return
    }
    throw new Error('Unconfirmed removal must never be submitted')
  })
  await page.goto('/admin/')
  await page
    .getByRole('button', { name: 'Selecionar projeto', exact: true })
    .click()
  await page
    .getByRole('button', { name: 'Remover projeto', exact: true })
    .click()
  await expect(page.getByText('REMOVE shop', { exact: true })).toBeVisible()
  await expect(
    page.getByText(/O backup final e a configuração ficarão/)
  ).toBeVisible()
  await expect(
    page.getByRole('button', { name: 'Confirmar e executar', exact: true })
  ).toBeDisabled()
  await page.screenshot({
    path: info.outputPath('removal-review.png'),
    fullPage: true
  })
})

test('recovery after restore verification shows the checkpoint and preserves later writes', async ({
  page
}, info) => {
  await mount(page)
  const job = {
    id: digest,
    request_key: id,
    operation: {
      kind: 'restore',
      project: 'shop',
      backup: 'nelcota-shop-pair.dump'
    },
    stage: 'needs_attention',
    sequence: 9,
    updated_at: 123,
    changed: true,
    backup: 'nelcota-shop-safety.dump',
    error: 'worker_interrupted',
    last_stage: 'verifying'
  }
  let stage = 'needs_attention',
    recoveries = 0
  await page.route('**/admin/api/updates', async (route) => {
    if (route.request().method() === 'GET') {
      await route.fulfill({ json: { result: 'status', data: base } })
      return
    }
    const body = route.request().postDataJSON()
    if (body.action === 'host_status') {
      await route.fulfill({
        json: {
          result: 'host_status',
          data: {
            ...host,
            jobs: [
              { ...job, stage, sequence: stage === 'needs_attention' ? 9 : 10 }
            ]
          }
        }
      })
      return
    }
    if (body.action === 'host_recover') {
      expect(body.job).toBe(digest)
      expect(body.confirmation).toBe('RESUME shop')
      recoveries++
      stage = 'accepted'
      await route.abort('failed')
      return
    }
    throw new Error('Recovery must never create a replacement restore request')
  })
  await page.goto('/admin/')
  await expect(page.getByText(/Última etapa registrada/)).toBeVisible()
  await page
    .getByRole('button', { name: 'Revisar recuperação', exact: true })
    .click()
  await expect(
    page.getByText(
      /A recuperação verifica o estado atual e preserva gravações posteriores/
    )
  ).toBeVisible()
  await expect(
    page.getByText(
      /Os dados e arquivos serão substituídos pelo par selecionado/
    )
  ).toHaveCount(0)
  await confirm(page, 'RESUME shop')
  await page.screenshot({
    path: info.outputPath('recovery-review.png'),
    fullPage: true
  })
  await page
    .getByRole('button', { name: 'Confirmar e executar', exact: true })
    .click()
  await expect(
    page.getByText('Pedido registrado', { exact: true })
  ).toBeVisible()
  expect(recoveries).toBe(1)
  await expect(
    page.getByRole('button', { name: 'Confirmar e executar', exact: true })
  ).toHaveCount(0)
})

test('rejected passwords settle the pending receipt and allow a fresh review', async ({
  page
}) => {
  await mount(page)
  let starts = 0
  await page.route('**/admin/api/updates', async (route) => {
    if (route.request().method() === 'GET') {
      await route.fulfill({ json: { result: 'status', data: base } })
      return
    }
    const body = route.request().postDataJSON()
    if (body.action === 'host_status') {
      await route.fulfill({ json: { result: 'host_status', data: host } })
      return
    }
    if (body.action === 'host_plan') {
      await route.fulfill({
        json: {
          result: 'host_plan',
          data: plan(body.operation, 'CREATE reporting')
        }
      })
      return
    }
    if (body.action === 'host_start') {
      starts++
      await route.fulfill({
        status: 403,
        json: { code: 'reauthentication_failed' }
      })
      return
    }
    throw new Error(`Unexpected action ${body.action}`)
  })
  await page.goto('/admin/')
  await page.getByLabel('Nome do projeto').fill('reporting')
  await page.getByLabel('Domínio da API').fill('api.reporting.test')
  await page
    .getByRole('button', { name: 'Validar e revisar', exact: true })
    .click()
  await confirm(page, 'CREATE reporting')
  await page
    .getByRole('button', { name: 'Confirmar e executar', exact: true })
    .click()
  await expect(page.getByRole('alert')).toContainText(
    'A operação foi bloqueada'
  )
  await expect(
    page.getByRole('button', { name: 'Validar e revisar', exact: true })
  ).toBeEnabled()
  expect(starts).toBe(1)
  expect(
    await page.evaluate(() =>
      sessionStorage.getItem('nelcota.host.request:operator@example.test')
    )
  ).toBeNull()
})

test('an older status request cannot settle a newly rejected request key', async ({
  page
}) => {
  await mount(page)
  let releaseOld!: () => void, releaseCurrent!: () => void
  const oldWait = new Promise<void>((resolve) => {
    releaseOld = resolve
  })
  const currentWait = new Promise<void>((resolve) => {
    releaseCurrent = resolve
  })
  let reads = 0,
    starts = 0,
    currentKey: string | null = null
  await page.route('**/admin/api/updates', async (route) => {
    if (route.request().method() === 'GET') {
      await route.fulfill({ json: { result: 'status', data: base } })
      return
    }
    const body = route.request().postDataJSON()
    if (body.action === 'host_status') {
      reads++
      if (reads === 2) {
        expect(body.key).toBeNull()
        await oldWait
      }
      if (reads >= 3 && body.key) {
        currentKey = body.key
        await currentWait
      }
      await route
        .fulfill({ json: { result: 'host_status', data: host } })
        .catch(() => {})
      return
    }
    if (body.action === 'host_plan') {
      await route.fulfill({
        json: {
          result: 'host_plan',
          data: plan(body.operation, 'CREATE reporting')
        }
      })
      return
    }
    if (body.action === 'host_start') {
      starts++
      await route.fulfill({
        status: 403,
        json: { code: 'reauthentication_failed' }
      })
      return
    }
    throw new Error(`Unexpected action ${body.action}`)
  })
  try {
    await page.goto('/admin/')
    await expect(page.getByLabel('Nome do projeto')).toBeVisible()
    await page.evaluate(() => window.dispatchEvent(new Event('focus')))
    await expect.poll(() => reads).toBe(2)
    await page.getByLabel('Nome do projeto').fill('reporting')
    await page.getByLabel('Domínio da API').fill('api.reporting.test')
    await page
      .getByRole('button', { name: 'Validar e revisar', exact: true })
      .click()
    await confirm(page, 'CREATE reporting')
    await page
      .getByRole('button', { name: 'Confirmar e executar', exact: true })
      .click()
    await expect(page.getByRole('alert')).toContainText(
      'Não foi possível confirmar a resposta'
    )
    releaseOld()
    await expect.poll(() => currentKey).not.toBeNull()
    const pending = await page.evaluate(() =>
      JSON.parse(
        sessionStorage.getItem('nelcota.host.request:operator@example.test') ??
          'null'
      )
    )
    expect(pending.key).toBe(currentKey)
    releaseCurrent()
    await expect(page.getByRole('alert')).toContainText(
      'A operação foi bloqueada'
    )
    expect(starts).toBe(1)
    expect(
      await page.evaluate(() =>
        sessionStorage.getItem('nelcota.host.request:operator@example.test')
      )
    ).toBeNull()
  } finally {
    releaseOld()
    releaseCurrent()
  }
})

test('a confirmed receipt releases the form even when local storage cleanup fails', async ({
  page
}) => {
  await page.addInitScript(() => {
    const original = Storage.prototype.removeItem
    Storage.prototype.removeItem = function (key: string) {
      if (key.startsWith('nelcota.host.request:'))
        throw new DOMException('Storage unavailable')
      return original.call(this, key)
    }
  })
  await mount(page)
  let accepted: Record<string, unknown> | null = null,
    starts = 0
  await page.route('**/admin/api/updates', async (route) => {
    if (route.request().method() === 'GET') {
      await route.fulfill({ json: { result: 'status', data: base } })
      return
    }
    const body = route.request().postDataJSON()
    if (body.action === 'host_status') {
      await route.fulfill({
        json: {
          result: 'host_status',
          data: {
            ...host,
            receipt: body.key ? accepted : null,
            jobs: accepted ? [accepted] : []
          }
        }
      })
      return
    }
    if (body.action === 'host_plan') {
      await route.fulfill({
        json: {
          result: 'host_plan',
          data: plan(body.operation, 'CREATE reporting')
        }
      })
      return
    }
    if (body.action === 'host_start') {
      starts++
      accepted = {
        id: digest,
        request_key: body.key,
        operation: body.operation ?? {
          kind: 'create',
          project: 'reporting',
          domain: 'api.reporting.test',
          profile: '1gb'
        },
        stage: 'completed',
        sequence: 7,
        updated_at: 123,
        changed: true,
        backup: null,
        error: null,
        last_stage: null
      }
      await route.fulfill({ json: { result: 'host_job', data: accepted } })
      return
    }
    throw new Error(`Unexpected action ${body.action}`)
  })
  await page.goto('/admin/')
  await page.getByLabel('Nome do projeto').fill('reporting')
  await page.getByLabel('Domínio da API').fill('api.reporting.test')
  await page
    .getByRole('button', { name: 'Validar e revisar', exact: true })
    .click()
  await confirm(page, 'CREATE reporting')
  await page
    .getByRole('button', { name: 'Confirmar e executar', exact: true })
    .click()
  await expect(page.getByRole('alert')).toContainText(
    'O resultado foi confirmado na VPS'
  )
  await expect(
    page.getByRole('button', { name: 'Confirmar e executar', exact: true })
  ).toHaveCount(0)
  await expect(
    page.getByRole('button', { name: 'Validar e revisar', exact: true })
  ).toBeEnabled()
  await page.reload()
  await expect(
    page.getByRole('button', { name: 'Validar e revisar', exact: true })
  ).toBeEnabled()
  expect(starts).toBe(1)
})

test('a fresh bounded history releases a completed operation that fell out of history', async ({
  page
}) => {
  await mount(page)
  let reads = 0
  const job = {
    id: digest,
    request_key: id,
    operation: {
      kind: 'create',
      project: 'reporting',
      domain: 'api.reporting.test',
      profile: '1gb'
    },
    stage: 'starting',
    sequence: 4,
    updated_at: 123,
    changed: true,
    backup: null,
    error: null,
    last_stage: null
  }
  await page.route('**/admin/api/updates', async (route) => {
    if (route.request().method() === 'GET') {
      await route.fulfill({ json: { result: 'status', data: base } })
      return
    }
    const body = route.request().postDataJSON()
    if (body.action === 'host_status') {
      reads++
      const jobs =
        reads === 1
          ? [job]
          : Array.from({ length: 32 }, (_, index) => ({
              ...job,
              request_key: (index + 40).toString(16).padStart(64, '0'),
              operation: { kind: 'backup', project: 'shop' },
              backup: `nelcota-shop-${index}.dump`,
              id: (index + 1).toString(16).padStart(64, '0'),
              stage: 'completed',
              sequence: 7
            }))
      await route.fulfill({
        json: { result: 'host_status', data: { ...host, jobs } }
      })
      return
    }
    throw new Error('No mutation is expected')
  })
  await page.goto('/admin/')
  await expect(
    page.getByRole('status').filter({ hasText: 'Iniciando serviços' })
  ).toBeVisible()
  await expect(
    page.getByRole('button', { name: 'Validar e revisar', exact: true })
  ).toBeDisabled()
  await page.evaluate(() => window.dispatchEvent(new Event('focus')))
  await expect(
    page.getByRole('button', { name: 'Validar e revisar', exact: true })
  ).toBeEnabled()
  await expect(
    page.getByRole('status').filter({ hasText: 'Iniciando serviços' })
  ).toHaveCount(0)
})

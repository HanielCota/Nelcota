import type { Page, Route } from '@playwright/test'

export async function editing(page: Page) {
  const assert = (value: unknown, message: string) => { if (!value) throw new Error(message) };
  const original = await page.evaluate(async () => (await fetch('/admin/api/tables/notes')).json());
  const reordered = {...original, rows:[...original.rows].reverse()};
  let patch!: { pk: Record<string, string>; values: Record<string, string | null> };
  const rowsHandler = async (route: Route) => {
    patch = route.request().postDataJSON();
    await route.fulfill({json:{count:1}});
  };
  await page.route('**/admin/api/tables/notes/rows', rowsHandler);
  const listHandler = async (route: Route) => {
    await new Promise(resolve => setTimeout(resolve, 700));
    await route.fulfill({json:reordered});
  };
  await page.route('**/admin/api/tables/notes?*', listHandler);
  await page.getByRole('button', {name:'Recarregar',exact:true}).click();
  await page.locator('[data-cell="0:1"]').dblclick();
  assert(await page.locator('#inline-editor').count() === 0, 'Editing must be blocked while refreshing');
  await page.waitForFunction(() => !document.querySelector('[aria-busy="true"]'));
  assert((await page.locator('[data-cell="0:0"]').innerText()).endsWith('012'), 'Delayed response must reorder rows');
  await page.locator('[data-cell="0:1"]').dblclick();
  await page.locator('#inline-editor').fill('Edited correct row');
  await page.locator('#inline-editor').press('Enter');
  await page.waitForFunction(() => !document.querySelector('[aria-busy="true"]'));
  assert(patch.pk.id.endsWith('012'), 'Save must address the edited primary key');
  assert(await page.locator('[data-cell="0:1"]').innerText() === 'Edited correct row', 'Local update must affect same row');
  await page.locator('[data-cell="0:2"]').dblclick();
  await page.locator('#inline-editor').press('Tab');
  assert(await page.getByRole('button',{name:'NULL',exact:true}).evaluate(el => el === document.activeElement), 'NULL must be keyboard reachable');
  await page.getByRole('button',{name:'NULL',exact:true}).press('Enter');
  await page.waitForFunction(() => !document.querySelector('[aria-busy="true"]'));
  assert(patch.values.body === null && patch.pk.id.endsWith('012'), 'Keyboard NULL must save correct value and key');
  await page.getByRole('button', {name:'Expandir linha 1',exact:true}).click();
  const beforeId = await page.locator('#f-id').inputValue();
  await page.locator('#f-title').fill('x'.repeat(80));
  await page.locator('#f-title').press('End');
  await page.locator('#f-title').press('x');
  assert(await page.locator('#f-title').inputValue() === 'x'.repeat(81), 'Long text must preserve input');
  assert(await page.locator('#f-title').evaluate(el => el === document.activeElement), 'Long text must retain focus');
  assert(await page.locator('#f-id').inputValue() === beforeId, 'Primary key must stay unchanged');
  await page.getByRole('button', {name:'Cancelar',exact:true}).click();
  assert(await page.getByRole('alertdialog').count() === 1, 'Dirty close must show confirmation');
  assert(await page.getByRole('dialog').count() === 1, 'Editor must remain open behind confirmation');
  await page.getByRole('button',{name:'Continuar editando',exact:true}).click();
  assert(await page.locator('#f-title').inputValue() === 'x'.repeat(81), 'Keeping edit must retain draft');
  await page.keyboard.press('Escape');
  assert(await page.getByRole('alertdialog').count() === 1, 'Escape must protect draft');
  await page.getByRole('button',{name:'Descartar alterações',exact:true}).click();
  await page.getByRole('dialog').waitFor({state:'hidden'});
  await page.unroute('**/admin/api/tables/notes?*', listHandler);
  await page.unroute('**/admin/api/tables/notes/rows', rowsHandler);
  console.log('PASS: delayed refresh, primary key identity, keyboard NULL, 81-character focus, dirty cancel/Escape');
}

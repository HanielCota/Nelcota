import type { Page, Route } from '@playwright/test'

export async function draftsAndFiles(page: Page) {
  const assert = (value: unknown, message: string) => { if (!value) throw new Error(message) };
  const passed = [];
  await page.evaluate(() => {
    localStorage.setItem('nelcota:sql-saved', JSON.stringify({v:1,items:[{id:'a',name:'Consulta A',sql:'select 1;',updatedAt:1},{id:'b',name:'Consulta B',sql:'select 2;',updatedAt:2}]}));
    localStorage.removeItem('nelcota:sql-workspace');
    localStorage.setItem('nelcota:sql-draft', 'select now();');
  });
  await page.reload();
  await page.getByRole('button',{name:'Consulta A',exact:true}).click();
  const editor = page.getByRole('textbox',{name:'Editor SQL',exact:true});
  await editor.click(); await editor.press('Control+a'); await page.keyboard.insertText('select 42; -- rascunho A');
  await page.getByRole('button',{name:'Consulta B',exact:true}).click();
  assert(await editor.innerText() === 'select 2;', 'Opening second query must work');
  await page.getByRole('button',{name:'Consulta A',exact:true}).click();
  assert(await editor.innerText() === 'select 42; -- rascunho A', 'Saved query draft must survive switching');
  await page.getByRole('button',{name:'Nova consulta',exact:true}).click();
  await editor.click(); await page.keyboard.insertText('select 7; -- rascunho solto');
  await page.getByRole('button',{name:'Criar tabela com RLS',exact:true}).click();
  await page.getByRole('button',{name:'select 7; -- rascunho solto',exact:true}).click();
  assert(await editor.innerText() === 'select 7; -- rascunho solto', 'Template must preserve loose draft');
  await page.reload();
  await editor.waitFor();
  assert(await editor.innerText() === 'select 7; -- rascunho solto', 'Active draft must survive browser refresh');
  await page.getByRole('button',{name:'Executar',exact:true}).click();
  await page.getByRole('button',{name:'Exportar',exact:true}).waitFor();
  const separator = page.getByRole('separator',{name:'Redimensionar editor e resultados'});
  const before = await separator.getAttribute('aria-valuenow');
  await separator.focus(); await separator.press('ArrowUp');
  assert(await separator.getAttribute('aria-valuenow') !== before, 'SQL resize must work by keyboard');
  passed.push('per-query and loose drafts, templates, refresh, SQL execution and keyboard resize');
  await page.goto('/admin/storage/documents');
  await page.getByRole('button',{name:'readme.txt',exact:true}).click();
  await page.getByText('Arquivo de exemplo\nPrévia em texto com acentos.',{exact:true}).waitFor();
  await page.getByRole('button',{name:'Fechar',exact:true}).first().click();
  await page.getByRole('button',{name:'screenshot.png',exact:true}).click();
  await page.getByRole('img',{name:'screenshot.png',exact:true}).waitFor();
  assert(await page.getByRole('img',{name:'screenshot.png',exact:true}).evaluate(el => el instanceof HTMLImageElement && el.complete && el.naturalWidth === 1), 'Image preview must load');
  await page.getByRole('button',{name:'Fechar',exact:true}).first().click();
  await page.getByRole('button',{name:'report.pdf',exact:true}).click();
  await page.getByRole('link',{name:'Abrir PDF',exact:true}).waitFor();
  assert((await page.getByRole('link',{name:'Abrir PDF',exact:true}).getAttribute('href'))?.startsWith('blob:'), 'PDF must use browser viewer');
  await page.getByRole('button',{name:'Fechar',exact:true}).first().click();
  passed.push('text/image/PDF previews');
  const pending: { uploaded?: { url: string; size?: number }; finish?: () => void } = {};
  const upload = async (route: Route) => {
    pending.uploaded = {url:route.request().url(),size:route.request().postDataBuffer()?.length};
    await new Promise<void>(resolve => { pending.finish = resolve });
    await route.fulfill({json:{}});
  };
  await page.route('**/admin/api/storage/buckets/documents/upload?*', upload);
  await page.locator('input[type=file]').setInputFiles({name:'new-file.txt',mimeType:'text/plain',buffer:Buffer.from('arquivo novo')});
  await page.getByRole('progressbar').waitFor();
  assert(await page.getByRole('progressbar').getAttribute('max') === '12', 'Upload progress must use byte size');
  await page.waitForFunction(() => document.querySelector('progress') !== null);
  while (!pending.finish) await new Promise(resolve => setTimeout(resolve, 20));
  pending.finish();
  await page.getByText('Enviado',{exact:true}).waitFor();
  assert(pending.uploaded?.size === 12 && new URL(pending.uploaded!.url).searchParams.get('name') === 'new-file.txt', 'Upload must preserve binary body and destination');
  assert(await page.getByRole('progressbar').count() === 0, 'Completed upload must leave pending progress state');
  await page.unroute('**/admin/api/storage/buckets/documents/upload?*', upload);
  passed.push('raw file upload and completion progress');
  return {passed};
}

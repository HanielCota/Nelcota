import type { Page, Route } from '@playwright/test'

export async function uploadsAndDetails(page: Page) {
  const assert = (value: unknown, message: string) => { if (!value) throw new Error(message) };
  await page.setViewportSize({width:1440,height:900});
  await page.goto('/admin/storage/documents');
  const uploads: string[] = [];
  const pending: { finish?: () => void } = {};
  const upload = async (route: Route) => {
    const url = new URL(route.request().url());
    uploads.push(url.search);
    if (url.searchParams.get('replace') === 'true') return route.fulfill({json:{}});
    await new Promise<void>(resolve => pending.finish = resolve);
    await route.fulfill({status:409,json:{error:'Already exists',code:'object_exists'}});
  };
  await page.route('**/admin/api/storage/buckets/documents/upload?*',upload);
  await page.getByRole('button',{name:'Enviar arquivos',exact:true}).waitFor();
  const drop = await page.evaluateHandle(() => {
    const data = new DataTransfer();
    data.items.add(new File(['dragged content'], 'existing.txt', {type:'text/plain'}));
    return data;
  });
  await page.getByRole('button',{name:'Arraste arquivos para cá ou clique para selecionar',exact:true}).dispatchEvent('drop',{dataTransfer:drop});
  await page.getByRole('progressbar').waitFor();
  while (!pending.finish) await new Promise(resolve => setTimeout(resolve,20));
  await page.getByRole('link',{name:'reports/',exact:true}).click();
  pending.finish();
  await page.getByRole('alertdialog').waitFor();
  assert((await page.getByRole('alertdialog').innerText()).includes('documents/'), 'Replace must identify original destination');
  await page.getByRole('button',{name:'Substituir',exact:true}).click();
  await page.getByRole('alertdialog').waitFor({state:'hidden'});
  assert(uploads.length === 2 && new URLSearchParams(uploads[1]).get('name') === 'existing.txt', 'Replace must retain original folder after navigation');
  await page.unroute('**/admin/api/storage/buckets/documents/upload?*',upload);
  await page.goto('/admin/sql');
  const long = 'Value with complete content\n' + 'x'.repeat(200);
  const sql = (route: Route) => route.fulfill({json:{results_truncated:false,results:[{columns:['body'],rows:[[long]],count:1,truncated:false}]}});
  await page.route('**/admin/api/sql',sql);
  await page.getByRole('button',{name:'Executar',exact:true}).click();
  await page.getByRole('button',{name:'Ver conteúdo completo',exact:true}).click();
  await page.getByRole('dialog').waitFor();
  assert((await page.getByRole('dialog').innerText()).includes(long), 'Long SQL result must show full content');
  await page.keyboard.press('Escape');
  await page.unroute('**/admin/api/sql',sql);
  return {passed:['drag and drop upload','conflict replacement retains original destination after navigation','long SQL result detail']};
}

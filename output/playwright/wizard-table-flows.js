async (page) => {
  const assert = (value, message) => { if (!value) throw new Error(message) };
  const choose = async (label, value) => { await page.getByRole('button', {name:label,exact:true}).click(); await page.getByRole('option',{name:value,exact:true}).click(); };
  await page.route('**/health', route => route.fulfill({contentType:'application/json',body:'{"status":"ok"}'}));
  const saved = [];
  let fail = true;
  await page.route('**/admin/api/tables', async route => {
    if(route.request().method()!=='POST') return route.fallback();
    const body = route.request().postDataJSON();
    if(!body.preview) {
      saved.push(body);
      if(fail) { fail=false; return route.fulfill({status:500,contentType:'application/json',body:JSON.stringify({error:'Falha de teste ao salvar'})}); }
    }
    return route.fulfill({contentType:'application/json',body:JSON.stringify({applied:!body.preview,catalog_pending:false,sql:['CREATE TABLE ...','CREATE POLICY ...']})});
  });
  const dialog = page.getByRole('dialog');
  await dialog.getByRole('button',{name:'Continuar',exact:true}).click();
  assert(await dialog.getByText('Preencha o nome para continuar.',{exact:true}).isVisible(),'Missing name validation');
  await dialog.getByRole('textbox',{name:'Nome da tabela',exact:true}).fill('minhas_tarefas');
  await choose('Começar com um exemplo','Tarefas');
  await dialog.getByRole('button',{name:'Continuar',exact:true}).click();
  await dialog.getByRole('heading',{name:'Quais informações cada registro terá?'}).waitFor();
  await page.screenshot({path:'D:/Nelcota/output/playwright/wizard-table-fields.png'});
  await dialog.getByRole('button',{name:'Voltar',exact:true}).click();
  assert(await dialog.getByRole('textbox',{name:'Nome da tabela',exact:true}).inputValue()==='minhas_tarefas','Back lost table name');
  await dialog.getByRole('button',{name:'Continuar',exact:true}).click();
  await dialog.getByRole('button',{name:'Adicionar campo',exact:true}).click();
  const input = dialog.getByRole('textbox',{name:'Nome do campo',exact:true}).last();
  await input.fill('titulo');
  await dialog.getByRole('button',{name:'Continuar',exact:true}).click();
  assert(await dialog.getByText('Já existe um campo com esse nome.',{exact:true}).count()===2,'Duplicate fields not validated');
  await input.fill('prioridade');
  await dialog.getByRole('button',{name:'Continuar',exact:true}).click();
  await choose('Quem pode acessar pelo app?','Cada usuário, apenas seus registros');
  await choose('O que essa pessoa pode fazer?','Ver, criar, editar e apagar');
  await dialog.getByRole('button',{name:'Continuar',exact:true}).click();
  assert(await dialog.getByText('user_id',{exact:true}).isVisible(),'Owner field not included in review');
  assert(await dialog.getByText('Cada usuário logado poderá ver, criar, editar e apagar apenas seus próprios registros.',{exact:true}).isVisible(),'Owner access summary inaccurate');
  assert(await dialog.getByRole('textbox',{name:'USING',exact:true}).count()===0,'SQL exposed in simple flow');
  await page.screenshot({path:'D:/Nelcota/output/playwright/wizard-table-review.png'});
  for (const width of [390,320]) {
    await page.setViewportSize({width,height:844});
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    const layout = await dialog.evaluate(element=>({width:element.clientWidth,scroll:element.scrollWidth}));
    assert(layout.scroll<=layout.width+1,`Sheet overflows at ${width}: ${JSON.stringify(layout)}`);
    const box = await dialog.getByRole('button',{name:'Criar tabela',exact:true}).boundingBox();
    assert(box && box.y+box.height<=844 && box.x+box.width<=width,`Footer inaccessible at ${width}`);
    await page.screenshot({path:`D:/Nelcota/output/playwright/wizard-table-review-${width}.png`});
  }
  await dialog.getByRole('button',{name:'Criar tabela',exact:true}).click();
  await dialog.getByRole('alert').filter({hasText:'Falha de teste ao salvar'}).waitFor();
  assert(await dialog.getByText('minhas_tarefas',{exact:true}).isVisible(),'Failure lost draft');
  await dialog.getByRole('button',{name:'Criar tabela',exact:true}).click();
  await dialog.waitFor({state:'hidden'});
  assert(saved.length===2,'Expected retry');
  const body=saved[1];
  assert(body.table.rls && body.policies[0].command==='all' && body.policies[0].check==='user_id = auth.uid()','Wrong owner policy');
  assert(body.table.columns.find(column=>column.name==='user_id').default==='auth.uid()','Owner default missing');
  assert(body.table.grants.find(grant=>grant.role==='anon').privileges.length===0,'Unexpected public access');
  console.log('PASS: table steps, example, back, duplicate validation, owner review, no SQL required, 320/390px layout, failure/retry, atomic request');
}

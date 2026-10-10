async (page) => {
  const assert = (value,message) => { if(!value) throw new Error(message); };
  const choose = async (label,value) => { await page.getByRole('button',{name:label,exact:true}).click(); await page.getByRole('option',{name:value,exact:true}).click(); };
  await page.setViewportSize({width:1440,height:900});
  const columns = ['id','user_id','author_id','title'].map(name=>({name,data_type:name==='title'?'text':'uuid',nullable:true,default:null,identity:null,generated:false,primary_key:name==='id',unique:null,references:null,comment:null}));
  await page.route('**/admin/api/tables/notes/structure', route=>route.fulfill({contentType:'application/json',body:JSON.stringify({name:'notes',comment:null,rls_enabled:false,primary_key:['id'],primary_key_constraint:'notes_pkey',columns,grants:[]})}));
  const saved=[];
  await page.route('**/admin/api/tables/notes/policies', async route=> {
    const body=route.request().postDataJSON(); if(!body.preview) saved.push(body);
    await route.fulfill({contentType:'application/json',body:JSON.stringify({applied:!body.preview,catalog_pending:false,sql:['CREATE POLICY ...','ALTER TABLE ... ENABLE ROW LEVEL SECURITY','GRANT SELECT ...']})});
  });
  const dialog=page.getByRole('dialog');
  await page.goto('http://127.0.0.1:5180/admin/policies');
  await page.getByRole('button',{name:'Nova política',exact:true}).first().click();
  await dialog.getByRole('button',{name:'Continuar',exact:true}).click();
  assert(await dialog.getByRole('button',{name:'Qual campo identifica o dono?',exact:true}).innerText()==='user_id','Owner guess wrong');
  await choose('O que essa pessoa pode fazer?','Ver, criar, editar e apagar');
  await choose('Qual campo identifica o dono?','author_id');
  await dialog.getByRole('button',{name:'Continuar',exact:true}).click();
  assert(await dialog.getByText('O dono será identificado pelo campo author_id. Os registros existentes precisam ter o ID do usuário nesse campo.',{exact:true}).isVisible(),'Owner review missing');
  await dialog.getByRole('textbox',{name:'Nome da regra',exact:true}).fill('owner_access');
  await dialog.getByRole('button',{name:'Criar regra de acesso',exact:true}).click();
  assert(await dialog.getByText('Já existe uma regra com esse nome. Escolha outro nome.',{exact:true}).isVisible(),'Existing name collision not shown');
  await dialog.getByRole('textbox',{name:'Nome da regra',exact:true}).fill('meus_registros');
  await dialog.getByRole('button',{name:'Voltar',exact:true}).click();
  assert((await dialog.getByRole('button',{name:'Qual campo identifica o dono?',exact:true}).innerText())==='author_id','Back lost selected column');
  await dialog.getByRole('button',{name:'Continuar',exact:true}).click();
  for (const width of [1440,390,320]) {
    await page.setViewportSize({width,height:900});
    await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
    const layout=await dialog.evaluate(el=>({width:el.clientWidth,scroll:el.scrollWidth}));
    assert(layout.scroll<=layout.width+1,`Policy sheet overflow at ${width}: ${JSON.stringify(layout)}`);
    const button=await dialog.getByRole('button',{name:'Criar regra de acesso',exact:true}).boundingBox();
    assert(button && button.x+button.width<=width && button.y+button.height<=900,'Create policy button clipped');
    await page.screenshot({path:`D:/Nelcota/output/playwright/wizard-policy-review-${width}.png`});
  }
  await dialog.getByRole('button',{name:'Criar regra de acesso',exact:true}).click();
  await dialog.waitFor({state:'hidden'});
  assert(saved[0].prepare_access && saved[0].policy.command==='all' && saved[0].policy.using==='author_id = auth.uid()' && saved[0].policy.check===saved[0].policy.using,'Wrong owner request');

  await page.setViewportSize({width:1440,height:900});
  await page.getByRole('button',{name:'Nova política',exact:true}).first().click();
  await choose('Quem pode acessar pelo app?','Qualquer pessoa, mesmo sem login');
  await dialog.getByRole('button',{name:'Continuar',exact:true}).click();
  await page.getByRole('button',{name:'O que essa pessoa pode fazer?',exact:true}).click();
  assert(await page.getByRole('option',{name:'Ver, criar, editar e apagar',exact:true}).getAttribute('data-disabled')!==null,'Public writes not disabled');
  await page.keyboard.press('Escape');
  await dialog.getByRole('button',{name:'Continuar',exact:true}).click();
  await dialog.getByRole('button',{name:'Criar regra de acesso',exact:true}).click();
  await dialog.waitFor({state:'hidden'});
  assert(saved[1].policy.command==='select' && saved[1].policy.roles.join(',')==='anon,authenticated' && !saved[1].policy.check,'Public access not read-only');

  await page.getByRole('button',{name:'Editar owner_access',exact:true}).first().click();
  assert(await dialog.getByRole('textbox',{name:'USING',exact:true}).inputValue()==='owner = auth.uid()','Existing SQL changed on opening edit');
  assert(!await dialog.getByRole('checkbox',{name:'Ativar proteção e configurar as permissões necessárias',exact:true}).isChecked(),'Editing unexpectedly changes access');
  await dialog.getByRole('textbox',{name:'USING',exact:true}).fill('owner = auth.uid() AND title <> \'\'');
  await dialog.getByRole('button',{name:'Cancelar',exact:true}).click();
  await page.getByRole('alertdialog').waitFor();
  await page.getByRole('button',{name:'Continuar editando',exact:true}).click();
  assert((await dialog.getByRole('textbox',{name:'USING',exact:true}).inputValue()).includes('AND title'),'Dirty cancel lost custom SQL');
  console.log('PASS: owner wizard, valid UUID picker, duplicate name, back, mobile footer, public reading, automatic grants, existing SQL preservation, dirty guard');
}

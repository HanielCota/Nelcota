async (page) => {
  const assert = (ok, text) => { if (!ok) throw new Error(text); };
  const passed = [];
  await page.setViewportSize({width: 1440, height: 900});
  await page.goto('http://127.0.0.1:5180/admin/');
  await page.getByRole('main').getByRole('link', {name: 'Novo usuário', exact: true}).click();
  const dialog = page.getByRole('dialog', {name: 'Novo usuário', exact: true});
  await dialog.waitFor();
  assert(!new URL(page.url()).searchParams.has('create'), 'Creation link must be consumed');
  await dialog.getByRole('textbox', {name: 'Email', exact: true}).fill('ana@');
  await dialog.getByRole('textbox', {name: 'Email', exact: true}).press('Tab');
  await dialog.getByText('Informe um email válido, como ana@exemplo.com.', {exact: true}).waitFor();
  assert(await dialog.getByRole('button', {name: 'Criar usuário', exact: true}).isDisabled(), 'Invalid email must not be submitted');
  await dialog.getByRole('textbox', {name: 'Email', exact: true}).fill('ana@example.test');
  await dialog.getByRole('button', {name: 'Gerar', exact: true}).click();
  assert(await dialog.getByRole('button', {name: 'Criar usuário', exact: true}).isEnabled(), 'Generated password and valid email must allow creation');
  await page.route('**/admin/api/users', route => route.request().method() === 'POST'
    ? route.fulfill({status: 409, json: {error: 'Este email já está cadastrado.'}}) : route.fallback());
  await dialog.getByRole('button', {name: 'Criar usuário', exact: true}).click();
  await dialog.getByRole('alert').getByText('Este email já está cadastrado.', {exact: true}).waitFor();
  assert(await dialog.getByRole('textbox', {name: 'Email', exact: true}).inputValue() === 'ana@example.test', 'Failed creation must retain the draft');
  await dialog.getByRole('button', {name: 'Cancelar', exact: true}).click();
  await page.getByRole('button', {name: 'Descartar alterações', exact: true}).click();
  passed.push('Overview shortcut, email validation, generated password, persistent error and retained draft');

  await page.goto('http://127.0.0.1:5180/admin/tables');
  const tablesSearch = page.getByRole('searchbox', {name: 'Buscar tabelas…'});
  await tablesSearch.fill('inexistente');
  await page.getByRole('complementary').getByText('Nenhum resultado para esses filtros', {exact: true}).waitFor();
  await page.getByRole('complementary').getByRole('button', {name: 'Limpar busca', exact: true}).first().click();
  assert(await tablesSearch.inputValue() === '', 'Clear must reset table search');
  assert(await tablesSearch.evaluate(e => document.activeElement === e), 'Clearing must keep focus in the search field');
  await page.getByRole('complementary').getByRole('link', {name: 'notes', exact: true}).waitFor();
  passed.push('Table search, empty search recovery and keyboard focus');

  await page.goto('http://127.0.0.1:5180/admin/policies?table=profiles');
  const policiesSearch = page.getByRole('searchbox', {name: 'Buscar tabela'});
  await policiesSearch.waitFor();
  assert(await policiesSearch.inputValue() === 'profiles', 'Incoming table link must filter policies');
  await policiesSearch.fill('notes');
  await page.waitForURL(/q=notes/);
  await page.getByRole('button', {name: 'Sem proteção', exact: true}).click();
  await page.waitForURL(/filter=disabled/);
  await page.reload();
  await page.getByRole('button', {name: 'Sem proteção', exact: true}).waitFor();
  assert(await policiesSearch.inputValue() === 'notes', 'Policy search must survive refresh');
  assert(await page.getByRole('button', {name: 'Sem proteção', exact: true}).getAttribute('aria-pressed') === 'true', 'Policy state must survive refresh');
  await page.getByRole('button', {name: 'Limpar filtros', exact: true}).click();
  await page.waitForURL('**/admin/policies');
  await page.getByRole('heading', {name: 'notes', exact: true}).waitFor();
  passed.push('Access filters, incoming links, refresh and clear filters');

  await page.goto('http://127.0.0.1:5180/admin/storage');
  await page.getByRole('searchbox', {name: 'Buscar buckets'}).fill('inexistente');
  await page.getByRole('main').getByText('Nenhum resultado para esses filtros', {exact: true}).waitFor();
  await page.getByRole('main').getByRole('button', {name: 'Limpar busca', exact: true}).first().click();
  await page.getByRole('main').getByRole('button', {name: 'Novo bucket', exact: true}).click();
  const bucketDialog = page.getByRole('dialog', {name: 'Novo bucket', exact: true});
  await bucketDialog.getByRole('textbox', {name: 'Nome', exact: true}).fill('public');
  await bucketDialog.getByText(/Os nomes public, sign e list são reservados/).waitFor();
  await bucketDialog.getByRole('textbox', {name: 'Nome', exact: true}).fill('avatars');
  await bucketDialog.getByRole('textbox', {name: 'Limite por arquivo (MB)', exact: true}).fill('-1');
  await bucketDialog.getByText('Informe um número maior que zero ou deixe vazio para usar o limite do servidor.', {exact: true}).waitFor();
  assert(await bucketDialog.getByRole('button', {name: 'Criar bucket', exact: true}).isDisabled(), 'Invalid limit must be explained and rejected');
  await bucketDialog.getByRole('textbox', {name: 'Limite por arquivo (MB)', exact: true}).fill('');
  assert(await bucketDialog.getByRole('button', {name: 'Criar bucket', exact: true}).isEnabled(), 'Optional empty limit must work');
  await bucketDialog.getByRole('button', {name: 'Cancelar', exact: true}).click();
  await page.getByRole('button', {name: 'Descartar alterações', exact: true}).click();
  passed.push('Bucket search, reserved names, limit validation and recovery');

  await page.setViewportSize({width: 390, height: 844});
  await page.goto('http://127.0.0.1:5180/admin/storage/documents');
  const download = page.getByRole('link', {name: 'Baixar readme.txt', exact: true});
  await download.waitFor();
  assert((await download.getAttribute('href')).includes('name=readme.txt'), 'Direct download must target the selected file');
  await page.getByRole('link', {name: 'Regras de acesso', exact: true}).click();
  assert(await page.getByRole('heading', {name: 'Quem acessa os arquivos', exact: true}).isVisible(), 'Access shortcut must reach the section on mobile');
  await page.getByRole('navigation', {name: 'Caminho no bucket'}).getByRole('link', {name: 'Arquivos', exact: true}).click();
  await page.getByRole('heading', {name: 'Arquivos', exact: true}).waitFor();
  passed.push('Mobile direct download, access shortcut and return to buckets');

  await page.goto('http://127.0.0.1:5180/admin/sql');
  await page.getByRole('button', {name: 'Consultas e modelos', exact: true}).click();
  await page.getByRole('menuitem', {name: 'Usuários recentes', exact: true}).click();
  const editor = page.getByRole('textbox', {name: 'Editor SQL', exact: true});
  assert((await editor.innerText()).includes('auth.users'), 'Mobile menu must open a query template');
  await page.getByRole('button', {name: 'Executar', exact: true}).click();
  const resultsTable = page.getByRole('region', {name: 'Resultados da consulta SQL', exact: true}).getByRole('table');
  await resultsTable.waitFor();
  assert(await resultsTable.getByRole('row').count() === 13, 'SQL results must show the twelve fixture rows and the header');
  passed.push('Mobile query templates and visible SQL results');

  await page.goto('http://127.0.0.1:5180/admin/connect');
  await page.getByRole('link', {name: 'Conectar meu app', exact: true}).click();
  await page.waitForFunction(() => {
    const top = document.getElementById('api-sdk').getBoundingClientRect().top;
    const mainTop = document.querySelector('main').getBoundingClientRect().top;
    return top >= mainTop - 1 && top < mainTop + 130;
  });
  const sdkTop = await page.locator('#api-sdk').evaluate(e => e.getBoundingClientRect().top);
  const mainTop = await page.getByRole('main').evaluate(e => e.getBoundingClientRect().top);
  assert(sdkTop >= mainTop - 1 && sdkTop < mainTop + 130, 'API shortcut must reach the SDK instructions');
  await page.getByRole('button', {name: 'JavaScript', exact: true}).click();
  await page.waitForURL(/lang=js/);
  assert(await page.getByRole('main').evaluate(e => e.scrollWidth <= e.clientWidth), 'API examples must fit the mobile width');
  passed.push('API shortcut, language selection and mobile overflow');
  return {passed};
}

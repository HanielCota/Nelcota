async (page) => {
  const failures = [], errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.route('**/health', route => route.fulfill({json:{status:'ok'}}));
  await page.evaluate(() => localStorage.setItem('mode-watcher-mode', 'dark'));
  const paths = ['', 'tables', 'tables/notes', 'tables/notes/structure', 'sql', 'migrations', 'storage', 'storage/documents', 'users', 'sign-in', 'policies', 'connect', 'projects'];
  const frames = () => page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  const assert = (condition, message) => { if (!condition) throw new Error(message); };
  for (const path of paths) {
    await page.setViewportSize({width:1440,height:900});
    await page.goto('http://127.0.0.1:5180/admin/' + path);
    await page.getByRole('heading', {level:1}).waitFor();
    await page.waitForFunction(() => !document.querySelector('main [data-slot="skeleton"]'));
    for (const width of [1440, 390]) {
      await page.setViewportSize({width,height:width === 390 ? 844 : 900});
      await frames();
      const overflow = await page.getByRole('main').evaluate(el => ({horizontal:el.scrollWidth > el.clientWidth, documentHorizontal:document.documentElement.scrollWidth > innerWidth, documentVertical:document.documentElement.scrollHeight > innerHeight}));
      if (Object.values(overflow).some(Boolean)) failures.push({path,width,...overflow});
      if (path === '' || path === 'projects' || path === 'connect') await page.screenshot({path:'output/playwright/layout-restored-dark-' + (path || 'overview') + '-' + width + '.png',scale:'css'});
    }
  }
  await page.setViewportSize({width:1440,height:900});
  await page.goto('http://127.0.0.1:5180/admin/');
  await page.getByRole('link',{name:'Nova tabela',exact:true}).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByLabel('Nome da tabela',{exact:true}).fill('notas_guiadas');
  await dialog.getByRole('button',{name:'Começar com um exemplo'}).click();
  await page.getByRole('option',{name:'Notas',exact:true}).click();
  await dialog.getByRole('button',{name:'Continuar',exact:true}).click();
  await dialog.getByRole('heading',{name:'Quais informações cada registro terá?'}).waitFor();
  assert(await dialog.getByLabel('Nome do campo',{exact:true}).count() === 2, 'Example fields must still be present');
  await dialog.getByRole('button',{name:'Continuar',exact:true}).click();
  await dialog.getByRole('heading',{name:'Quem vai usar esses dados?'}).waitFor();
  await dialog.getByRole('button',{name:'Continuar',exact:true}).click();
  await dialog.getByRole('heading',{name:'Conferir antes de criar'}).waitFor();
  for (const width of [1440,390,320]) {
    await page.setViewportSize({width,height:width < 768 ? 844 : 900});
    await frames();
    const buttons = await dialog.locator('[data-slot="sheet-footer"] button').evaluateAll(nodes => nodes.map(node => { const box=node.getBoundingClientRect();return {label:node.textContent.trim(),visible:box.width > 30 && box.height > 30 && box.left >= 0 && box.right <= innerWidth && box.top >= 0 && box.bottom <= innerHeight}; }));
    assert(buttons.length === 3 && buttons.every(button => button.visible), 'Table wizard footer must fit at ' + width);
    if (width === 390) await page.screenshot({path:'output/playwright/layout-restored-table-guide-mobile.png',scale:'css'});
  }
  await dialog.getByRole('button',{name:'Cancelar',exact:true}).click();
  await page.getByRole('button',{name:'Descartar alterações',exact:true}).click();
  await dialog.waitFor({state:'hidden'});
  await page.setViewportSize({width:1440,height:900});
  await page.goto('http://127.0.0.1:5180/admin/policies');
  await page.getByRole('button',{name:'Nova política',exact:true}).first().click();
  await dialog.getByRole('button',{name:'Continuar',exact:true}).click();
  await dialog.getByRole('heading',{name:'Defina o acesso aos registros'}).waitFor();
  assert(await dialog.getByRole('checkbox',{name:'Adicionar um campo para identificar o dono'}).isChecked(), 'Owner guidance must still supply missing field');
  await dialog.getByRole('button',{name:'Continuar',exact:true}).click();
  await dialog.getByRole('heading',{name:'Conferir antes de criar'}).waitFor();
  await frames();
  await page.screenshot({path:'output/playwright/layout-restored-policy-guide.png',scale:'css'});
  await dialog.getByRole('button',{name:'Cancelar',exact:true}).click();
  return {darkLayouts:paths.length * 2, failures, errors, guides:'table 4 steps and policy 3 steps preserved; table footer fits 320, 390 and 1440px'};
}

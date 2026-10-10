async (page) => {
  const assert = (condition, message) => { if (!condition) throw new Error(message); };
  const results = [];
  for (const theme of ['light', 'dark']) {
    await page.evaluate(theme => document.documentElement.classList.toggle('dark', theme === 'dark'), theme);
    await page.keyboard.press('Tab');
    const cell = page.getByRole('gridcell', {name:'Planejamento da semana', exact:true}).first();
    await cell.focus();
    const focus = await cell.evaluate(el => {
      const css = getComputedStyle(el);
      return {visible:el.matches(':focus-visible'), outline:css.outlineColor};
    });
    assert(focus.visible && focus.outline.includes('0 0'), `${theme}: grid focus is not neutral`);
    await page.keyboard.press('Enter');
    const editor = page.locator('#inline-editor');
    await editor.waitFor();
    const ring = await editor.evaluate(el => getComputedStyle(el.parentElement).getPropertyValue('--tw-ring-color'));
    assert(ring.includes('0 0'), `${theme}: inline editor frame is not neutral: ${ring}`);
    await page.screenshot({path:`D:/Nelcota/output/playwright/focus-grid-${theme}.png`});
    await page.keyboard.press('Escape');
    const select = page.getByRole('button', {name:'Linhas por página', exact:true});
    await select.focus();
    const selectFocus = await select.evaluate(el => ({visible:el.matches(':focus-visible'), ring:getComputedStyle(el).getPropertyValue('--tw-ring-color')}));
    assert(selectFocus.visible && selectFocus.ring.includes('0 0'), `${theme}: select focus is not neutral`);
    results.push({theme, cell:focus, editorRing:ring, select:selectFocus});
  }
  await page.route('**/admin/api/users?*', route => route.abort('failed'));
  await page.route('**/admin/api/users', route => route.abort('failed'));
  await page.getByRole('navigation', {name:'Navegação principal'}).getByRole('link', {name:'Usuários', exact:true}).click();
  await page.getByRole('alert').filter({hasText:'Sem resposta do servidor'}).waitFor();
  const errorFocus = await page.getByRole('main').evaluate(el => ({focused:document.activeElement === el, outline:getComputedStyle(el).outlineStyle}));
  assert(errorFocus.focused && errorFocus.outline === 'none', 'Server error state still frames the page');
  await page.screenshot({path:'D:/Nelcota/output/playwright/focus-server-error-dark.png'});
  return {results, errorFocus};
}

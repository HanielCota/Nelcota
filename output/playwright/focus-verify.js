async (page) => {
  const inspect = async (locator) => locator.evaluate(el => {
    const css = getComputedStyle(el);
    return {
      focused: document.activeElement === el,
      visible: el.matches(':focus-visible'),
      outline: css.outlineStyle,
      outlineWidth: css.outlineWidth,
      outlineColor: css.outlineColor,
      borderColor: css.borderColor,
      ring: css.getPropertyValue('--tw-ring-color'),
      shadow: css.boxShadow,
    };
  });
  const assert = (condition, message) => { if (!condition) throw new Error(message); };
  const results = [];
  for (const theme of ['light', 'dark']) {
    await page.evaluate(theme => document.documentElement.classList.toggle('dark', theme === 'dark'), theme);
    const nav = page.getByRole('navigation', {name: 'Navegação principal'});
    for (const name of ['Banco', 'Arquivos', 'Usuários', 'Acesso', 'API', 'Visão geral']) {
      await nav.getByRole('link', {name, exact:true}).click();
      await page.getByRole('main').waitFor();
      const main = await inspect(page.getByRole('main'));
      assert(main.focused && main.outline === 'none', `${theme}: mouse navigation to ${name} framed the page: ${JSON.stringify(main)}`);
    }
    await page.keyboard.press('Tab');
    await nav.getByRole('link', {name:'Usuários', exact:true}).focus();
    await page.keyboard.press('Enter');
    await page.getByRole('searchbox', {name:'Buscar usuários por email'}).waitFor();
    const keyboardMain = await inspect(page.getByRole('main'));
    assert(keyboardMain.focused && keyboardMain.visible && keyboardMain.outline === 'none', `${theme}: keyboard route focus failed: ${JSON.stringify(keyboardMain)}`);
    const search = page.getByRole('searchbox', {name:'Buscar usuários por email'});
    await search.click();
    const field = await inspect(search);
    assert(field.visible && field.ring.includes('0 0'), `${theme}: search focus is not neutral: ${JSON.stringify(field)}`);
    await page.keyboard.press('Tab');
    const button = await inspect(page.getByRole('button', {name:'Novo usuário', exact:true}));
    assert(button.focused && button.visible && button.ring.includes('0 0'), `${theme}: keyboard button focus failed: ${JSON.stringify(button)}`);
    await page.getByRole('link', {name:'Pular para o conteúdo'}).focus();
    await page.keyboard.press('Enter');
    const skipped = await inspect(page.getByRole('main'));
    assert(skipped.focused && skipped.outline === 'none', `${theme}: skip link framed the page`);
    await page.screenshot({path:`D:/Nelcota/output/playwright/focus-users-${theme}.png`});
    results.push({theme, keyboardMain, field, button, skipLink: 'passed', mouseRoutes:6});
  }
  return results;
}

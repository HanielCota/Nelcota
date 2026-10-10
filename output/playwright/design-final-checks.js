async (page) => {
  const assert = (ok, text) => { if (!ok) throw new Error(text); };
  const settle = () => page.evaluate(() => Promise.all(document.getAnimations().filter(animation => Number.isFinite(animation.effect?.getTiming().iterations)).map(animation => animation.finished.catch(() => {}))));
  const layouts = [];
  for (const width of [768, 1024]) {
    await page.setViewportSize({width, height: 900});
    await page.goto('http://127.0.0.1:5175/admin/');
    await page.getByRole('slider').waitFor();
    assert(await page.getByRole('main').evaluate(el => el.scrollWidth <= el.clientWidth), 'Tablet content fits');
    assert(await page.locator('body').evaluate(el => el.scrollWidth <= innerWidth), 'Tablet navigation fits');
    layouts.push(width);
  }
  await page.setViewportSize({width: 1440, height: 900});
  await page.goto('http://127.0.0.1:5175/admin/users?create=true');
  const user = page.getByRole('dialog', {name: 'Novo usuário', exact: true});
  await user.waitFor();
  await settle();
  await page.screenshot({path: 'output/playwright/design-dialog-user.png', scale: 'css'});
  await page.setViewportSize({width: 390, height: 844});
  await settle();
  await page.screenshot({path: 'output/playwright/design-dialog-user-mobile.png', scale: 'css'});
  await user.getByRole('button', {name: 'Cancelar', exact: true}).click();
  await page.getByRole('dialog').waitFor({state: 'hidden'});
  await page.setViewportSize({width: 1440, height: 900});
  await page.goto('http://127.0.0.1:5175/admin/storage');
  await page.getByRole('button', {name: 'Novo bucket', exact: true}).click();
  await page.getByRole('dialog', {name: 'Novo bucket', exact: true}).waitFor();
  await settle();
  await page.screenshot({path: 'output/playwright/design-dialog-bucket.png', scale: 'css'});
  await page.getByRole('dialog').getByRole('button', {name: 'Cancelar', exact: true}).click();
  await page.route('**/admin/api/session', route => route.fulfill({status: 401, json: {error: 'Unauthorized'}}));
  await page.goto('http://127.0.0.1:5175/admin/');
  await page.getByRole('textbox', {name: 'Email', exact: true}).waitFor();
  for (const theme of ['dark', 'light']) {
    if ((await page.locator('html').getAttribute('class') ?? '').includes('dark') !== (theme === 'dark')) {
      await page.getByRole('button', {name: theme === 'dark' ? 'Tema escuro' : 'Tema claro', exact: true}).click();
    }
    for (const width of [1440, 390]) {
      await page.setViewportSize({width, height: width === 390 ? 844 : 900});
      await settle();
      assert(await page.getByRole('main').evaluate(el => el.scrollWidth <= el.clientWidth), 'Login fits');
      await page.screenshot({path: `output/playwright/design-final-${theme}-login${width === 390 ? '-mobile' : ''}.png`, scale: 'css'});
    }
  }
  return {tabletWidths: layouts, loginThemes: ['dark', 'light'], dialogs: ['user desktop/mobile', 'bucket desktop']};
}

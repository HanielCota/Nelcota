async (page) => {
  const paths = ['', 'tables', 'tables/notes', 'tables/notes/structure', 'sql', 'migrations', 'storage', 'storage/documents', 'users', 'sign-in', 'policies', 'connect', 'projects'];
  const results = [];
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  const settle = () => page.evaluate(() => Promise.all(document.getAnimations().filter(animation => Number.isFinite(animation.effect?.getTiming().iterations)).map(animation => animation.finished.catch(() => {}))));
  for (const theme of ['dark', 'light']) {
    for (const path of paths) {
      await page.setViewportSize({width: 1440, height: 900});
      await page.goto('http://127.0.0.1:5175/admin/' + path);
      await page.getByRole('heading', {level: 1}).waitFor();
      await page.waitForFunction(() => !document.querySelector('main [data-slot="skeleton"]'));
      if ((await page.locator('html').getAttribute('class') ?? '').includes('dark') !== (theme === 'dark')) {
        await page.getByRole('button', {name: 'Conta', exact: true}).click();
        await page.getByRole('menuitem', {name: theme === 'dark' ? 'Tema escuro' : 'Tema claro', exact: true}).click();
      }
      await page.waitForFunction(dark => document.documentElement.classList.contains('dark') === dark, theme === 'dark');
      const key = path.replaceAll('/', '-') || 'overview';
      for (const width of [1440, 390]) {
        await page.setViewportSize({width, height: width === 390 ? 844 : 900});
        await page.mouse.move(2, 2);
        await settle();
        const info = await page.getByRole('main').evaluate(el => ({
          horizontalOverflow: el.scrollWidth > el.clientWidth,
          documentOverflow: document.documentElement.scrollHeight > innerHeight,
        }));
        results.push({path: key, theme, width, ...info});
        await page.screenshot({path: `output/playwright/design-final-${theme}-${key}${width === 390 ? '-mobile' : ''}.png`, scale: 'css'});
      }
    }
  }
  return {checked: results.length, overflow: results.filter(result => result.horizontalOverflow || result.documentOverflow), pageErrors: errors};
}

async (page) => {
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  const paths = ['', 'tables', 'tables/notes', 'tables/notes/structure', 'sql', 'migrations', 'storage', 'storage/documents', 'users', 'sign-in', 'policies', 'connect', 'projects'];
  const failures = [];
  for (const path of paths) {
    const key = path.replaceAll('/', '-') || 'overview';
    await page.setViewportSize({width: 1440, height: 900});
    await page.goto('http://127.0.0.1:5180/admin/' + path);
    await page.getByRole('heading', {level: 1}).waitFor();
    await page.waitForFunction(() => !document.querySelector('main [data-slot="skeleton"]'));
    await page.mouse.move(2, 2);
    await page.screenshot({path: 'output/playwright/ux-after-' + key + '.png', scale: 'css'});
    for (const width of [1440, 768, 390, 320]) {
      await page.setViewportSize({width, height: width < 768 ? 844 : 900});
      await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
      const sizes = await page.getByRole('main').evaluate(el => ({
        horizontal: el.scrollWidth > el.clientWidth,
        documentVertical: document.documentElement.scrollHeight > innerHeight,
        documentHorizontal: document.documentElement.scrollWidth > innerWidth,
        width: el.clientWidth,
        scrollWidth: el.scrollWidth,
      }));
      if (sizes.horizontal || sizes.documentVertical || sizes.documentHorizontal) failures.push({path: key, viewport: width, ...sizes});
      if (width === 390) await page.screenshot({path: 'output/playwright/ux-after-' + key + '-mobile.png', scale: 'css'});
    }
  }
  return {screens: paths.length * 4, failures, errors};
}

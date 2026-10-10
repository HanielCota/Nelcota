async (page) => {
  await page.route('**/health', route => route.fulfill({status: 200, body: 'ok'}));
  await page.evaluate(() => localStorage.setItem('mode-watcher-mode', 'dark'));
  const pages = ['', 'tables', 'tables/notes', 'tables/notes/structure', 'sql', 'migrations', 'storage', 'storage/documents', 'users', 'sign-in', 'policies', 'connect', 'projects'];
  const results = [];
  for (const path of pages) {
    await page.setViewportSize({width: 1440, height: 900});
    await page.goto('http://127.0.0.1:5175/admin/' + path);
    await page.getByRole('heading', {level: 1}).waitFor({state: 'visible'});
    await page.waitForFunction(() => !document.querySelector('main [data-slot="skeleton"]'));
    const key = path.replaceAll('/', '-') || 'overview';
    await page.mouse.move(2, 2);
    await page.screenshot({path: 'output/playwright/design-after-' + key + '.png', scale: 'css'});
    for (const width of [1440, 390]) {
      await page.setViewportSize({width, height: width === 390 ? 844 : 900});
      const info = await page.getByRole('main').evaluate(el => ({
        heading: el.querySelector('h1')?.textContent,
        text: el.innerText.slice(0, 240),
        horizontalOverflow: el.scrollWidth > el.clientWidth,
        documentOverflow: document.documentElement.scrollHeight > innerHeight,
        actions: Array.from(el.querySelectorAll('button,a')).filter(node => node.getBoundingClientRect().width > 0).slice(0, 18).map(node => node.textContent.trim() || node.getAttribute('aria-label')),
      }));
      results.push({path: path || 'overview', width, ...info});
      if (width === 390) await page.screenshot({path: 'output/playwright/design-after-' + key + '-mobile.png', scale: 'css'});
    }
  }
  return results;
}

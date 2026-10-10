async (page) => {
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.addInitScript(() => {
    localStorage.setItem('nelcota.locale', 'pt-BR');
    localStorage.setItem('mode-watcher-mode', 'dark');
  });
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  let releaseSession;
  const pendingSession = new Promise(resolve => { releaseSession = resolve; });
  await page.route('**/admin/api/session', async route => {
    await pendingSession;
    await route.fulfill({ status: 401, json: { error: 'Unauthorized' } });
  });
  await page.route('**/admin/api/whoami', route => route.fulfill({ json: { project: 'Nelcota' } }));
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto('http://127.0.0.1:5175/admin/', { waitUntil: 'domcontentloaded' });
  await page.getByRole('status').waitFor({ state: 'visible' });
  await page.evaluate(() => document.fonts.ready);
  const statusText = await page.getByRole('status').innerText();
  if (statusText.trim() !== 'Carregando…') throw new Error(`Unexpected status: ${statusText}`);
  const logoLoaded = await page.locator('main img').evaluate(img => img.complete && img.naturalWidth > 0);
  if (!logoLoaded) throw new Error('Logo did not load');
  await page.screenshot({ path: 'output/playwright/loading-minimal-desktop-dark.png' });
  await page.evaluate(() => document.documentElement.classList.remove('dark'));
  await page.screenshot({ path: 'output/playwright/loading-minimal-desktop-light.png' });
  await page.setViewportSize({ width: 390, height: 844 });
  await page.evaluate(() => document.documentElement.classList.add('dark'));
  const layout = await page.locator('main > div').evaluate(el => {
    const rect = el.getBoundingClientRect();
    return {
      centered: Math.abs(rect.x + rect.width / 2 - innerWidth / 2) < 1 && Math.abs(rect.y + rect.height / 2 - innerHeight / 2) < 1,
      noOverflow: document.documentElement.scrollWidth === innerWidth && document.documentElement.scrollHeight === innerHeight,
    };
  });
  if (!layout.centered || !layout.noOverflow) throw new Error(`Invalid mobile layout: ${JSON.stringify(layout)}`);
  await page.screenshot({ path: 'output/playwright/loading-minimal-mobile-dark.png' });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  const animation = await page.locator('.loading-screen-dot').first().evaluate(el => getComputedStyle(el).animationName);
  if (animation !== 'none') throw new Error(`Reduced motion did not disable animation: ${animation}`);
  releaseSession();
  await page.locator('input[type="email"]').waitFor({ state: 'visible' });
  if (await page.locator('main[aria-busy="true"]').count()) throw new Error('Loading screen stayed after the session completed');
  if (errors.length) throw new Error(`Browser errors: ${errors.join('; ')}`);
  console.log(JSON.stringify({ statusText, logoLoaded, mobile: layout, reducedMotion: animation, loginTransition: true, browserErrors: errors }));
}

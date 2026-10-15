"""Live Chromium panel/SDK probes; run only inside its owned staging container."""
import asyncio
from collections import Counter, deque
import json
import math
import os
from pathlib import Path
import time
import urllib.parse

from playwright.async_api import async_playwright, Error


def require(condition, reason):
    if not condition:
        raise RuntimeError(reason)


def save(path, value):
    temporary = path.with_suffix('.writing')
    temporary.write_text(json.dumps(value, indent=2, allow_nan=False)+'\n')
    temporary.replace(path)


async def main():
    fixture = json.loads(Path('/fixture.json').read_text())
    output = Path('/evidence')
    base = fixture['base']
    origin = urllib.parse.urlsplit(base)
    require(origin.scheme == 'https' and origin.hostname == 'localhost', 'browser_requires_owned_verified_local_HTTPS')
    require(Path.home() == Path('/home/ubuntu'), 'unexpected_container_home')
    require(900 <= fixture['session_seconds'] <= 1500, 'invalid_browser_session_budget')
    state = {'phase':'startup', 'api_requests':0, 'decoded_network_bytes':0,
             'external_requests_blocked':0, 'page_errors':0}
    paths, grants, consumers = Counter(), Counter(), Counter()
    arrivals = deque()
    def fatal(reason):
        save(output/'browser-critical.json', {'reason':reason, **state})
        os._exit(77)  # PID 1 exit also stops its private PID namespace children.
    def request_seen(request):
        parsed = urllib.parse.urlsplit(request.url)
        if parsed.path.startswith('/admin/api/') or parsed.path.startswith('/auth/') or parsed.path.startswith('/rest/'):
            state['api_requests'] += 1
            paths[parsed.path] += 1
            now = time.monotonic()
            arrivals.append(now)
            while arrivals and arrivals[0] < now-1:
                arrivals.popleft()
            if state['api_requests'] > 12000 or len(arrivals) > 64:
                fatal('browser_request_budget')
        if parsed.path == '/auth/v1/token':
            grant = urllib.parse.parse_qs(parsed.query).get('grant_type', ['unknown'])[0]
            if grant not in ('password', 'refresh_token'):
                fatal('unexpected_browser_auth_grant')
            grants[grant] += 1
    def received(event):
        size = event.get('dataLength')
        if isinstance(size, bool) or not isinstance(size, (int, float)) or not math.isfinite(size) or size < 0:
            fatal('invalid_browser_network_evidence')
        state['decoded_network_bytes'] += size
        if state['decoded_network_bytes'] > 128*1024*1024:
            fatal('browser_traffic_byte_budget')
    def response_seen(response):
        parsed = urllib.parse.urlsplit(response.url)
        if parsed.path.startswith(('/auth/', '/rest/')):
            upstream = response.headers.get('x-g05-upstream')
            if upstream not in fixture['upstreams']:
                fatal('missing_auth_consumer_evidence')
            consumers[str(fixture['upstreams'].index(upstream))] += 1
    async def route(request):
        parsed = urllib.parse.urlsplit(request.request.url)
        if (parsed.scheme, parsed.netloc) != (origin.scheme, origin.netloc):
            state['external_requests_blocked'] += 1
            await request.abort('blockedbyclient')
            return
        if parsed.path.startswith('/g05-sdk/'):
            path = (Path('/runtime/sdk')/parsed.path.removeprefix('/g05-sdk/')).resolve()
            if not path.is_relative_to('/runtime/sdk') or not path.is_file() or path.suffix != '.js' or path.stat().st_size > 2*1024*1024:
                fatal('invalid_owned_SDK_asset')
            await request.fulfill(path=str(path), content_type='text/javascript')
            return
        await request.continue_()
    async def navigate(page, table, marker):
        start = await page.evaluate('performance.now()')
        await page.evaluate('(table) => {history.pushState(null,"","/admin/tables/"+table); dispatchEvent(new PopStateEvent("popstate"));}', table)
        await page.wait_for_function('(marker) => document.querySelector(\'[data-cell="0:1"]\')?.textContent?.includes(marker)', arg=marker, timeout=10000)
        await page.evaluate('() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)))')
        elapsed = await page.evaluate('performance.now()')-start
        require(math.isfinite(elapsed) and elapsed > 0, 'invalid_visual_feedback_evidence')
        return elapsed
    async def sdk_read(page):
        value = await page.evaluate('''async ({user,body}) => {
          const result=await window.g05Client.from('bench_private').select('id,user_id,body').order('id').limit(20);
          return !result.error && Array.isArray(result.data) && result.data.length===20 && result.data.every((row,index)=>row.id===2*(index+1)&&row.user_id===user&&row.body===body);
        }''', {'user':fixture['user'], 'body':fixture['label']})
        require(value, 'live_browser_SDK_RLS_oracle')
    plateaus, timings = [], []
    async with async_playwright() as playwright:
        browser = await playwright.chromium.launch(executable_path='/runtime/chrome/chromium', headless=True,
            chromium_sandbox=False, args=['--disable-gpu', '--disable-background-networking', '--no-first-run'])
        try:
            context = await browser.new_context(locale='pt-BR', ignore_https_errors=False, service_workers='block', viewport={'width':1440, 'height':900})
            await context.route('**/*', route)
            page = await context.new_page()
            profilers = []
            async def instrument(page):
                profiler = await context.new_cdp_session(page)
                await profiler.send('Network.enable')
                profiler.on('Network.dataReceived', received)
                profilers.append(profiler)
            await instrument(page)
            if fixture.get('untrusted_control'):
                try:
                    await page.goto(base+'/admin/', timeout=15000)
                except Error as error:
                    require('ERR_CERT_AUTHORITY_INVALID' in str(error), 'untrusted_browser_TLS_wrong_rejection')
                    save(output/'browser-result.json', {'status':'passed', 'scope':'untrusted CA negative control only', 'untrusted_CA_rejected':True})
                    await context.close()
                    return
                raise RuntimeError('untrusted_browser_CA_accepted')
            context.on('request', request_seen)
            context.on('response', response_seen)
            def page_error(_):
                state['page_errors'] += 1
            context.on('page', lambda page: page.on('pageerror', page_error))
            page.on('pageerror', page_error)
            state['phase'] = 'live-panel-login'
            await page.goto(base+'/admin/tables/bench_products', timeout=15000)
            await page.get_by_label('Email', exact=True).fill(fixture['admin_email'])
            await page.get_by_label('Senha', exact=True).fill(fixture['password'])
            await page.get_by_role('button', name='Entrar', exact=True).click()
            await page.wait_for_function('() => document.querySelector(\'[data-cell="0:1"]\')?.textContent?.includes("project-0:")', timeout=15000)
            pages = [page, await context.new_page(), await context.new_page()]
            for page in pages[1:]:
                await instrument(page)
                await page.goto(base+'/admin/tables/bench_products', timeout=15000)
            for page in pages:
                await page.evaluate('''async ({base,key}) => {
                  const sdk=await import('/g05-sdk/index.js'); window.g05SDK=sdk;
                  window.g05Client=sdk.createClient(base,{auth:{storageKey:key}});
                }''', {'base':base, 'key':fixture['storage_key']})
            signin = await pages[0].evaluate('''async ({email,password,user}) => {
              const value=await window.g05Client.auth.signInWithPassword({email,password});
              return {valid:!value.error&&value.data?.user?.id===user,expires:value.data?.expires_in};
            }''', {key:fixture[key] for key in ['email', 'password', 'user']})
            require(signin['valid'] and not isinstance(signin['expires'], bool) and isinstance(signin['expires'], int) and 0 < signin['expires'] <= fixture['session_seconds']-120, 'actual_auth_TTL_exceeds_browser_profile')
            await asyncio.sleep(.5)
            for page in pages:
                await sdk_read(page)
            before = grants['refresh_token']
            state['phase'] = 'simultaneous-SDK-refresh'
            refreshed = await asyncio.gather(*[page.evaluate('''async () => {
              const values=await Promise.all(Array.from({length:16},()=>window.g05Client.auth.refreshSession()));
              const token=values[0]?.data?.access_token;
              if(!token||values.some(value=>value.error||value.data?.access_token!==token)) return null;
              const bytes=new TextEncoder().encode(token);const digest=await crypto.subtle.digest('SHA-256',bytes);
              return Array.from(new Uint8Array(digest)).map(value=>value.toString(16).padStart(2,'0')).join('');
            }''') for page in pages])
            require(refreshed[0] and len(set(refreshed)) == 1 and grants['refresh_token']-before == 1, 'cross_tab_refresh_did_not_coalesce')
            auth_started = time.monotonic()
            initial = await pages[0].evaluate('''async () => {const value=await window.g05Client.auth.getSession();window.g05InitialAccess=value?.access_token;return Boolean(value?.access_token);}''')
            require(initial, 'browser_SDK_session_missing')
            async def sample(label):
                values = []
                for profiler in profilers:
                    await profiler.send('HeapProfiler.collectGarbage')
                    dom = await profiler.send('Memory.getDOMCounters')
                    heap = await profiler.send('Runtime.getHeapUsage')
                    counters = {'documents':dom['documents'], 'nodes':dom['nodes'], 'listeners':dom['jsEventListeners'], 'heap_bytes':heap['usedSize']}
                    require(all(not isinstance(value, bool) and isinstance(value, (int, float)) and math.isfinite(value) and value >= 0 for value in counters.values()), 'invalid_browser_owner_evidence')
                    require(counters['documents'] > 0 and counters['nodes'] > 0 and 0 < counters['heap_bytes'] <= 512*1024*1024, 'browser_page_heap_budget')
                    values.append(counters)
                result = {'phase':label, 'seconds_since_auth':time.monotonic()-auth_started, 'pages':values}
                plateaus.append(result)
                save(output/'browser-plateaus.json', plateaus)
            for page in pages:
                await navigate(page, 'bench_alternate', 'alternate:')
                await navigate(page, 'bench_products', 'project-0:')
            await sample('baseline')
            for cycle in range(5):
                state['phase'] = 'navigation-pressure-'+str(cycle+1)
                deadline = time.monotonic()+30
                count = 0
                while time.monotonic() < deadline:
                    started = time.monotonic()
                    page = pages[count%3]
                    table, marker = 'bench_products', 'project-0:'
                    if count%2:
                        table, marker = 'bench_alternate', 'alternate:'
                    timings.append(await navigate(page, table, marker))
                    require(not state['page_errors'], 'live_panel_page_error')
                    count += 1
                    await asyncio.sleep(max(0,.1-(time.monotonic()-started)))
                for page in pages:
                    await page.evaluate('''async ({base,key}) => {
                      for(let index=0;index<20;index++){const client=window.g05SDK.createClient(base,{auth:{storageKey:key}});await client.auth.getSession();client.dispose();window.g05Disposed=new WeakRef(client);}
                    }''', {'base':base, 'key':fixture['storage_key']})
                await asyncio.sleep(15)
                await sample('navigation-rest-'+str(cycle+1))
                for page in pages:
                    require(await page.evaluate('window.g05Disposed.deref()===undefined'), 'disposed_SDK_client_retained')
                save(output/'browser-checkpoint.json', {**state, 'completed_cycles':cycle+1})
            state['phase'] = 'real-credential-expiry'
            renewals = grants['refresh_token']
            reads = 0
            while time.monotonic()-auth_started < fixture['session_seconds']:
                started = time.monotonic()
                for page in pages:
                    await sdk_read(page)
                    reads += 1
                require(not state['page_errors'], 'live_panel_page_error')
                save(output/'browser-checkpoint.json', {**state, 'sdk_RLS_reads':reads, 'seconds_since_auth':time.monotonic()-auth_started, 'refresh_requests':grants['refresh_token']})
                await asyncio.sleep(max(0,1-(time.monotonic()-started)))
            require(grants['refresh_token'] > renewals, 'no_actual_browser_credential_renewal')
            require(set(consumers) == {'0', '1'}, 'both_real_auth_consumers_not_observed')
            expired = await pages[0].evaluate('''async () => {
              const response=await fetch('/rest/v1/bench_private?limit=1',{headers:{Authorization:'Bearer '+window.g05InitialAccess}});
              return response.status;
            }''')
            require(expired == 401, 'actually_expired_access_not_rejected')
            state['phase'] = 'shared-SDK-signout'
            signed_out = await pages[0].evaluate('''async () => {window.g05Last=await window.g05Client.auth.getSession();const value=await window.g05Client.auth.signOut();return !value.error;}''')
            require(signed_out, 'browser_signout_failed')
            await asyncio.sleep(1)
            for page in pages:
                require(await page.evaluate('(async ()=>(await window.g05Client.auth.getSession())===null)()'), 'cross_tab_logout_state_retained')
            revoked = await pages[0].evaluate('''async () => {
              const response=await fetch('/auth/v1/token?grant_type=refresh_token',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({refresh_token:window.g05Last.refresh_token})});
              return response.status;
            }''')
            require(revoked == 400, 'logout_did_not_revoke_refresh')
            for page in pages:
                await page.evaluate('window.g05Client.dispose()')
            await sample('final-disposed')
            for index in range(3):
                baseline = plateaus[0]['pages'][index]
                final = plateaus[-1]['pages'][index]
                require(final['heap_bytes'] <= baseline['heap_bytes']+8*1024*1024, 'browser_heap_growth_budget')
                require(final['nodes'] <= baseline['nodes']+250 and final['listeners'] <= baseline['listeners']+32, 'browser_owner_growth_budget')
            timings.sort()
            p95 = timings[max(0,math.ceil(len(timings)*.95)-1)]
            require(p95 <= 1000 and not state['external_requests_blocked'] and not state['page_errors'], 'browser_visual_or_external_traffic_gate')
            result = {'status':'passed', 'scope':'real local HTTPS panel and built TypeScript SDK, not public WAN capacity',
                      'browser_version':browser.version, 'tabs':3, 'pressure_rest_cycles':5, 'navigation_samples':len(timings),
                      'authoritative_visual_feedback_p95_ms':p95, 'feedback_SLO_ms':1000, 'api_requests':state['api_requests'],
                      'decoded_network_bytes':state['decoded_network_bytes'], 'auth_grants':dict(grants), 'auth_consumers':dict(consumers),
                      'sdk_RLS_reads':reads, 'cross_tab_refresh_coalesced':True, 'actual_credential_renewal':True,
                      'actual_expired_access_status':expired,
                      'actual_expires_in_seconds':signin['expires'], 'auth_observation_seconds':time.monotonic()-auth_started,
                      'logout_refresh_status':revoked, 'access_logout_contract':'Existing access remains valid until its expiry; no immediate access revocation claim',
                      'disposed_clients_collected':True, 'external_requests_blocked':state['external_requests_blocked'], 'page_errors':state['page_errors']}
            for profiler in profilers:
                await profiler.detach()
            await context.close()
            save(output/'browser-result.json', result)
        finally:
            await browser.close()


if __name__ == '__main__':
    output = Path('/evidence')
    (output/'driver-ready').touch()
    start_deadline = time.monotonic()+30
    while not (output/'driver-start').exists():
        if time.monotonic() > start_deadline:
            raise RuntimeError('Browser owner did not arm the traffic start gate')
        time.sleep(.1)
    try:
        asyncio.run(main())
    except Exception as error:
        save(output/'browser-error.json', {'type':type(error).__name__})
        raise
    (output/'driver-finished').touch()
    deadline = time.monotonic()+30
    while not (output/'driver-release').exists():
        if time.monotonic() > deadline:
            raise RuntimeError('Browser completion was not acknowledged by its owner')
        time.sleep(.1)

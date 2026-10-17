"""Strict numeric publication oracle for concurrent browser/backend auth."""
import math

PROFILE = {'cycles':5, 'accounts_per_cycle':8, 'requests_per_cycle':112,
           'maximum_inflight':8, 'maximum_arrivals_per_second':16,
           'maximum_response_bytes':65536, 'maximum_cycle_seconds':40,
           'mutation_retries':0, 'login_p95_ms':750, 'login_p99_ms':1500,
           'read_refresh_p95_ms':200, 'read_refresh_p99_ms':500,
           'scope':'Concurrent with existing navigation pressure, two real auth consumers, verified owned HTTPS. SMTP/OAuth and public ingress remain separate.'}
COUNTS = {'signup':8, 'login':8, 'invalid_password':8, 'user':8, 'rls':16,
          'refresh':32, 'reuse_rejected':8, 'family_revoked':8, 'logout':8,
          'logout_refresh_rejected':8}
SUCCESS = {name:COUNTS[name] for name in ['signup','login','user','rls','refresh','logout']}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def finite(value):
    return type(value) in {int,float} and math.isfinite(value)


def cycle(value, index):
    require(isinstance(value,dict) and value.get('status')=='passed' and type(value.get('cycle')) is int
            and value['cycle']==index and 0<=index<5, 'Missing successful auth pressure cycle')
    for key in ['requests_attempted','requests_completed','independent_browser_request_count']:
        require(type(value.get(key)) is int and value[key]==112, 'Auth pressure request census differs')
    require(type(value.get('maximum_inflight')) is int and 1<=value['maximum_inflight']<=8
            and type(value.get('maximum_arrivals_per_second')) is int and 1<=value['maximum_arrivals_per_second']<=16,
            'Auth pressure exceeded its admission/arrival boundary')
    require(type(value.get('accounts_created')) is int and value['accounts_created']==8
            and type(value.get('mutation_retries')) is int and value['mutation_retries']==0,
            'Auth fixture/mutation accounting changed')
    require(finite(value.get('elapsed_seconds')) and 0<value['elapsed_seconds']<40,
            'Auth pressure exceeded its monotonic cycle deadline')
    require(type(value.get('response_bytes_consumed')) is int and 0<value['response_bytes_consumed']<=112*65536,
            'Invalid auth pressure response-byte accounting')
    counts=value.get('counts')
    require(isinstance(counts,dict) and set(counts)==set(COUNTS)
            and all(type(counts[name]) is int and counts[name]==expected for name,expected in COUNTS.items()),
            'Auth expected positive/rejection denominators differ')
    latencies=value.get('successful_latency')
    require(isinstance(latencies,dict) and set(latencies)==set(SUCCESS), 'Missing successful-only auth latency classes')
    for name,expected in SUCCESS.items():
        row=latencies[name]
        require(isinstance(row,dict) and set(row)=={'samples','p50_ms','p95_ms','p99_ms'}
                and type(row['samples']) is int and row['samples']==expected
                and all(finite(row[key]) for key in ['p50_ms','p95_ms','p99_ms'])
                and 0<row['p50_ms']<=row['p95_ms']<=row['p99_ms'], 'Invalid successful auth latency evidence')
        if name=='login':
            require(row['p95_ms']<=750 and row['p99_ms']<=1500, 'Login missed its declared SLO')
        if name in {'refresh','user','rls'}:
            require(row['p95_ms']<=200 and row['p99_ms']<=500, 'Read/refresh missed its declared SLO')
    fields=['cycle','status','requests_attempted','requests_completed','independent_browser_request_count',
            'maximum_inflight','maximum_arrivals_per_second','accounts_created','counts',
            'successful_latency','elapsed_seconds','response_bytes_consumed','mutation_retries']
    return {key:value[key] for key in fields}


def complete(values):
    require(isinstance(values,list) and len(values)==5, 'Missing complete five-cycle auth pressure evidence')
    return [cycle(value,index) for index,value in enumerate(values)]

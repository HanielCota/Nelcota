"""Native k6 regression for the byte oracle; this is not product load evidence."""
import argparse
from http.server import BaseHTTPRequestHandler,HTTPServer
import hashlib
import json
import os
from pathlib import Path
import secrets
import shutil
import subprocess
import sys
import threading
import time

from campaign import K6,docker
from ownership import OwnedDocker
from watchdog import LABEL,atomic_json,cleanup,host_reserves,process_identity


class Fixture(HTTPServer):
    def get_request(self):
        connection,address=super().get_request()
        connection.settimeout(1)
        return connection,address


class Handler(BaseHTTPRequestHandler):
    def log_message(self,*args):pass

    def respond(self,status,body):
        time.sleep(.005)  # One worker, at most 200 fixture responses/second.
        self.server.counts[str(status)]=self.server.counts.get(str(status),0)+1
        self.send_response(status)
        size=len(body)
        if self.server.mode=='truncated' and self.command=='GET' and status==200:size=19
        self.send_header('Content-Length',str(size))
        self.send_header('Connection','close')
        self.end_headers()
        self.wfile.write(body)
        self.close_connection=True

    def do_GET(self):
        if self.path.startswith('/rest/v1/bench_private'):
            self.respond(401,b'{}')
            return
        if self.path!='/storage/v1/object/public/bench/fixture.txt':
            self.respond(404,b'{}')
            return
        self.server.reads+=1
        body=b'"benchmark payload"'
        if self.server.mode=='corrupt':body=b'!'*len(body)
        if self.server.mode=='truncated':body=body[:10]
        self.respond(200,body)

    def write_request(self):
        size=int(self.headers.get('Content-Length','0'))
        if not 0<=size<=1024:
            self.respond(413,b'{}')
            return
        self.rfile.read(size)
        self.respond(200,b'{}')

    do_POST=write_request
    do_PUT=write_request


def run(output,baseline=None):
    output=output.resolve()
    if output.exists() and any(output.iterdir()):raise ValueError('Preserve previous evidence')
    output.mkdir(parents=True,exist_ok=True)
    sources=[Path(__file__).with_name('campaign.js')]
    if baseline is not None:sources.append(baseline)
    for source in sources:
        if not source.is_file() or not 0<source.stat().st_size<=128*1024:
            raise ValueError('Oracle script must be a bounded local JavaScript file')
    if host_reserves(output):raise RuntimeError('Host reserves do not allow the oracle check')
    identity='nelcota-oracle-'+secrets.token_hex(16)
    owned=OwnedDocker(identity,docker)
    cases=[('candidate-valid','valid',0),('candidate-corrupt','corrupt',108),('candidate-truncated','truncated',108)]
    if baseline is not None:cases.append(('baseline-corrupt','corrupt',0))
    began=time.monotonic()
    observed=began
    phase='preparation'
    stop=threading.Event()
    manifest={'identity':identity,'output':str(output),'reserve_path':str(output),'pid':os.getpid(),'process_identity':process_identity(os.getpid()),'started_monotonic':began,'maximum_seconds':90,'preparation_seconds':90,'containers':[identity+'-'+label for label,_,_ in cases],'volumes':[]}
    atomic_json(output/'ownership.json',manifest)
    def heart():
        while not stop.is_set():
            atomic_json(output/'heartbeat.json',{'phase':phase,'monotonic':time.monotonic(),'resource_monotonic':observed})
            stop.wait(1)
    heartbeat=threading.Thread(target=heart,daemon=True)
    heartbeat.start()
    watchdog=subprocess.Popen([sys.executable,str(Path(__file__).with_name('watchdog.py')),str(output/'ownership.json')],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,start_new_session=True)
    server=Fixture(('127.0.0.1',0),Handler)
    server.mode='valid';server.reads=0;server.counts={}
    thread=threading.Thread(target=server.serve_forever,kwargs={'poll_interval':.05},daemon=True)
    thread.start()
    reports=[]
    passed=False
    try:
        current=output/'candidate.js'
        shutil.copyfile(Path(__file__).with_name('campaign.js'),current)
        fixture={'base':'http://127.0.0.1:'+str(server.server_port),'object_bytes':17,'service':secrets.token_urlsafe(32),'invalid_access':secrets.token_urlsafe(32),'user':secrets.token_hex(16)}
        for label,mode,expected in cases:
            phase='preparation'
            source=current
            if label=='baseline-corrupt':
                source=output/'baseline.js'
                shutil.copyfile(baseline,source)
            server.mode=mode;server.reads=0;server.counts={}
            name=identity+'-'+label
            owned('run','-d','--name',name,'--label',LABEL+'='+identity,'--network','host','--cpus','.25','--cpuset-cpus',str(sorted(os.sched_getaffinity(0))[-1]),'--memory','128m','--memory-swap','128m','--pids-limit','64','--user',str(os.getuid())+':'+str(os.getgid()),'-v',str(source)+':/campaign.js:ro','-e','FIXTURE','-e','WORKLOAD=storage','-e','DURATION=2s','-e','VUS=1',K6,'run','--quiet','--summary-mode=disabled','--no-thresholds','/campaign.js',env={'FIXTURE':json.dumps(fixture)})
            deadline=time.monotonic()+15
            phase='oracle'
            while True:
                state=json.loads(owned('inspect',name))[0]['State']
                observed=time.monotonic()
                if not state['Running']:break
                if time.monotonic()>deadline or watchdog.poll() is not None or host_reserves(output):raise RuntimeError('Oracle safety deadline or reserve breach')
                time.sleep(.1)
            logs=owned('logs','--tail','50',name,include_stderr=True)
            (output/(label+'.log')).write_text(logs)
            result={'case':label,'mode':mode,'expected_exit_code':expected,'actual_exit_code':state['ExitCode'],'positive_object_reads':server.reads,'responses':dict(server.counts),'oom_killed':state['OOMKilled'],'script_sha256':hashlib.sha256(source.read_bytes()).hexdigest()}
            result['passed']=state['ExitCode']==expected and server.reads>0 and not state['OOMKilled']
            reports.append(result)
            atomic_json(output/'oracle-results.json',{'scope':'native k6 oracle regression against a controlled HTTP fixture; no product or endurance certification','cases':reports})
            if not result['passed']:raise RuntimeError('Native byte-oracle control failed: '+label)
        passed=True
    finally:
        cleanup(manifest)
        server.shutdown();server.server_close();thread.join(timeout=5)
        clean=not owned('ps','-aq','--filter','label='+LABEL+'='+identity) and not thread.is_alive()
        atomic_json(output/'cleanup.json',{'confirmed':clean})
        if clean:(output/'watchdog-disarmed').touch()
        stop.set();heartbeat.join(timeout=3);watchdog.wait(timeout=15)
        atomic_json(output/'completion.json',{'status':'failed','certifies_g05':False,'scope':'oracle regression only','cleanup_confirmed':clean})
        if passed and clean:atomic_json(output/'completion.json',{'status':'passed','certifies_g05':False,'scope':'oracle regression only','cleanup_confirmed':True})
    if not clean:raise RuntimeError('Native oracle fixture cleanup failed')
    return reports


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--baseline-script',type=Path)
    args=parser.parse_args()
    print(json.dumps(run(args.output,args.baseline_script),indent=2))

"""Independent topology comparisons cannot certify incomplete or regressed work."""
from pathlib import Path
import json
import math
import sys
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'bench'))
import host_compare


class HostComparison(unittest.TestCase):
    def setUp(self):
        self.directory=tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.before=Path(self.directory.name)/'before'
        self.after=Path(self.directory.name)/'after'
        for root in [self.before,self.after]:
            root.mkdir()
            profile={field:1 for field in host_compare.FIELDS}
            profile.update(schema=2,projects=1,seconds_per_step=300,nominal_rate_per_project=8,certifies_five_minute_steps=True)
            (root/'manifest.json').write_text(json.dumps(profile))
            (root/'completion.json').write_text(json.dumps({'status':'passed','cleanup_confirmed':True}))
            (root/'cleanup.json').write_text(json.dumps({'confirmed':True}))
            (root/'resource-violations.json').write_text('[]')
            intervals=[]
            for index in range(1,4):
                metrics={'unexpected':{'value':0},'checks':{'fails':0},'overload_rejections':{'count':0},'iterations':{'count':2400},'dropped_iterations':{'count':0}}
                for operation in host_compare.OPERATIONS:
                    metrics['latency_'+operation]={'p(95)':10,'p(99)':20}
                    metrics['throughput_'+operation]={'rate':4}
                (root/('balanced-'+str(index)+'.json')).write_text(json.dumps({'project-0':{'schema':2,'metrics':metrics}}))
                (root/('balanced-'+str(index)+'-gates.json')).write_text('[]')
                intervals.append({'phase':'balanced-'+str(index),'completed':True,'requested_seconds':300,'elapsed_seconds':301})
            (root/'traffic-ledger.jsonl').write_text(''.join(json.dumps(item)+'\n' for item in intervals))

    def test_equivalent_three_samples_pass(self):
        self.assertTrue(host_compare.compare(self.before,self.after)['gate_passed'])

    def test_one_operation_regression_cannot_hide_in_total_throughput(self):
        for index in range(1,4):
            path=self.after/('balanced-'+str(index)+'.json')
            value=json.loads(path.read_text())
            value['project-0']['metrics']['throughput_write']['rate']=3
            path.write_text(json.dumps(value))
        self.assertFalse(host_compare.compare(self.before,self.after)['gate_passed'])

    def test_failed_or_missing_interval_cannot_certify(self):
        (self.after/'traffic-ledger.jsonl').write_text('')
        with self.assertRaises(ValueError):
            host_compare.compare(self.before,self.after)
        (self.after/'completion.json').write_text(json.dumps({'status':'failed','cleanup_confirmed':True}))
        with self.assertRaises(ValueError):
            host_compare.compare(self.before,self.after)

    def test_nonfinite_success_rate_cannot_certify(self):
        path=self.after/'balanced-1.json'
        value=json.loads(path.read_text())
        value['project-0']['metrics']['throughput_read']['rate']=math.nan
        path.write_text(json.dumps(value))
        with self.assertRaises(ValueError):
            host_compare.compare(self.before,self.after)

    def test_nonfinite_counts_and_durations_cannot_hide_underdelivery(self):
        path=self.after/'balanced-1.json'
        original=path.read_text()
        for count in [math.nan,math.inf,-1,True]:
            value=json.loads(original)
            value['project-0']['metrics']['dropped_iterations']['count']=count
            path.write_text(json.dumps(value))
            with self.assertRaises(ValueError):host_compare.compare(self.before,self.after)
        path.write_text(original)
        ledger=self.after/'traffic-ledger.jsonl'
        records=[json.loads(line) for line in ledger.read_text().splitlines()]
        records[0]['elapsed_seconds']=math.nan
        ledger.write_text(''.join(json.dumps(item)+'\n' for item in records))
        with self.assertRaises(ValueError):host_compare.compare(self.before,self.after)

    def test_matching_bad_p99_cannot_certify_from_an_empty_gate_file(self):
        for root in [self.before,self.after]:
            path=root/'balanced-1.json'
            value=json.loads(path.read_text())
            value['project-0']['metrics']['latency_read']['p(99)']=501
            path.write_text(json.dumps(value))
        with self.assertRaises(ValueError):host_compare.compare(self.before,self.after)

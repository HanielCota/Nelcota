"""Owned staging must survive name collisions, process reuse and concurrent receipts."""
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
import json
import math
import os
import signal
import subprocess
import sys
import tempfile
import threading
import unittest
from unittest import mock

ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'bench'))


class OwnershipBoundaries(unittest.TestCase):
    def test_postgres_child_oom_cannot_hide_behind_a_running_postmaster(self):
        from host_resources import require_no_oom
        require_no_oom({'oom':0,'oom_kill':0})
        for evidence in [{},{'oom':0},{'oom':False,'oom_kill':0}]:
            with self.assertRaises(ValueError):require_no_oom(evidence)
        with self.assertRaises(RuntimeError):require_no_oom({'oom':1,'oom_kill':1})

    def test_foreign_name_is_never_removed(self):
        from ownership import OwnedDocker
        raw=mock.Mock()
        owned=OwnedDocker('this-run',raw)
        with mock.patch('ownership.container_id',return_value=None):
            for command in ['rm','stop','start','kill','pause','unpause','update']:
                with self.assertRaises(RuntimeError):owned(command,'same-reused-name')
        raw.assert_not_called()

    def test_verified_container_is_removed_by_id_after_a_name_reuse(self):
        from ownership import OwnedDocker
        raw=mock.Mock()
        with mock.patch('ownership.container_id',return_value='a'*64):
            OwnedDocker('this-run',raw)('rm','-fv','same-reused-name')
        self.assertEqual(raw.call_args.args,('rm','-fv','a'*64))

    def test_existing_foreign_volume_is_not_relabelled_or_mounted(self):
        from ownership import OwnedDocker
        raw=mock.Mock(return_value=json.dumps([{'Labels':{'io.nelcota.stress.run':'another-run'}}]))
        with self.assertRaises(RuntimeError):
            OwnedDocker('this-run',raw)('volume','create','--label','io.nelcota.stress.run=this-run','colliding-volume')
        self.assertEqual([call.args[:2] for call in raw.call_args_list],[('volume','inspect')])

    def test_atomic_receipts_allow_independent_concurrent_writers(self):
        from watchdog import atomic_json
        original=Path.replace
        barrier=threading.Barrier(4)
        def together(source,target):
            barrier.wait(timeout=5)
            return original(source,target)
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'receipt.json'
            with mock.patch.object(Path,'replace',together):
                with ThreadPoolExecutor(max_workers=4) as workers:
                    list(workers.map(lambda value:atomic_json(path,{'writer':value}),range(4)))
            self.assertIn(json.loads(path.read_text())['writer'],range(4))
            self.assertEqual(list(Path(directory).glob('*.writing')),[])


@unittest.skipUnless(hasattr(os,'pidfd_open') and hasattr(signal,'pidfd_send_signal'),'Linux pidfd is required')
class EmergencyTrafficStop(unittest.TestCase):
    def child(self):
        process=subprocess.Popen([sys.executable,'-c','import time; time.sleep(10)'])
        def finish():
            if process.poll() is None:process.terminate()
            process.wait(timeout=5)
        self.addCleanup(finish)
        return process

    def test_pidfd_stops_owned_traffic_without_a_docker_request(self):
        from watchdog import atomic_json,process_identity,stop_generator
        process=self.child()
        identifier='a'*64
        original=Path.read_text
        def read(path,*args,**kwargs):
            if str(path)==f'/proc/{process.pid}/cgroup':
                return '0::/docker/'+identifier+'\n'
            return original(path,*args,**kwargs)
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            atomic_json(root/'generator-process.json',{'identity':'this-run','pid':process.pid,'process_identity':process_identity(process.pid),'container_id':identifier})
            with mock.patch.object(Path,'read_text',read),mock.patch('watchdog.subprocess.run',side_effect=AssertionError('Docker must not be needed')):
                self.assertTrue(stop_generator({'identity':'this-run','output':directory}))
            self.assertEqual(process.wait(timeout=5),-signal.SIGKILL)

    def test_other_run_or_reused_pid_is_never_signalled(self):
        from watchdog import atomic_json,process_identity,stop_generator
        process=self.child()
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            atomic_json(root/'generator-process.json',{'identity':'other-run','pid':process.pid,'process_identity':process_identity(process.pid),'container_id':'a'*64})
            self.assertFalse(stop_generator({'identity':'this-run','output':directory}))
            atomic_json(root/'generator-process.json',{'identity':'this-run','pid':process.pid,'process_identity':'1','container_id':'a'*64})
            self.assertFalse(stop_generator({'identity':'this-run','output':directory}))
            self.assertIsNone(process.poll())


class HostMemoryEvidence(unittest.TestCase):
    def test_clean_page_cache_is_reclaimable_but_tmpfs_is_not(self):
        import host_resources
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            (root/'memory.max').write_text('1000')
            stat=root/'memory.stat'
            stat.write_text('anon 100\nfile 900\nshmem 100\nkernel 50\nfile_dirty 0\nfile_writeback 0\n')
            with mock.patch.object(host_resources,'CGROUP',root):
                self.assertFalse(host_resources.memory_reserve_breached())
                stat.write_text('anon 100\nfile 900\nshmem 800\nkernel 50\nfile_dirty 0\nfile_writeback 0\n')
                self.assertTrue(host_resources.memory_reserve_breached())


class FaultTransitionEvidence(unittest.TestCase):
    def test_short_fault_returning_to_none_is_still_a_new_observation_epoch(self):
        from observations import ObservationEpoch
        window=ObservationEpoch()
        epoch,value=window.capture()
        self.assertIsNone(value)
        self.assertFalse(window.changed(epoch))
        window.set({'container':'owned-app','name':'restart'})
        window.set(None)
        self.assertIsNone(window.capture()[1])
        self.assertTrue(window.changed(epoch))

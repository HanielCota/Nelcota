"""Owned presigned redirects cannot leak a fixture bearer or cookie."""
from pathlib import Path
import secrets
import sys
import unittest
import urllib.parse
import urllib.request
import uuid

ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'bench'))
from http_boundary import FixtureRedirect


class FixtureRedirects(unittest.TestCase):
    def setUp(self):
        self.handler=FixtureRedirect('http://127.0.0.1:9100')
        self.query=urllib.parse.urlencode({'X-Amz-Algorithm':'AWS4-HMAC-SHA256','X-Amz-Signature':secrets.token_hex(32)})
        self.path='/benchmark-files/bench/'+str(uuid.uuid4())
        self.request=urllib.request.Request('http://127.0.0.1:8000/storage/v1/object/bench/fixture.txt',headers={'Authorization':'Bearer '+secrets.token_urlsafe(32),'Cookie':'fixture='+secrets.token_urlsafe(32)})

    def test_owned_presigned_read_drops_application_credentials(self):
        url='http://127.0.0.1:9100'+self.path+'?'+self.query
        forwarded=self.handler.redirect_request(self.request,None,302,'Found',{},url)
        self.assertEqual(forwarded.full_url,url)
        self.assertFalse(forwarded.has_header('Authorization'))
        self.assertFalse(forwarded.has_header('Cookie'))
        self.assertTrue(self.request.has_header('Authorization'))

    def test_external_rebound_or_unsigned_target_is_rejected(self):
        for url in ['https://example.test'+self.path+'?'+self.query,'http://127.0.0.1:9101'+self.path+'?'+self.query,'http://127.0.0.1:9100/admin?'+self.query,'http://127.0.0.1:9100'+self.path,'http://operator@127.0.0.1:9100'+self.path+'?'+self.query]:
            with self.assertRaises(RuntimeError):
                self.handler.redirect_request(self.request,None,302,'Found',{},url)

    def test_ordinary_fixture_request_never_follows_a_redirect(self):
        self.assertIsNone(FixtureRedirect().redirect_request(self.request,None,302,'Found',{},'http://127.0.0.1:9100'+self.path+'?'+self.query))

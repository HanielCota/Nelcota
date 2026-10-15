"""Never forward fixture authorization through an untrusted redirect."""
import urllib.parse
import urllib.request
import uuid


class FixtureRedirect(urllib.request.HTTPRedirectHandler):
    max_redirections=1
    max_repeats=1

    def __init__(self,s3_endpoint=None):
        self.endpoint=None
        if s3_endpoint is not None:
            endpoint=urllib.parse.urlsplit(s3_endpoint)
            if endpoint.scheme not in ('http','https') or endpoint.hostname not in ('127.0.0.1','localhost') or endpoint.username is not None or endpoint.password is not None or endpoint.path not in ('','/') or endpoint.query or endpoint.fragment:
                raise ValueError('Invalid owned S3 redirect endpoint')
            self.endpoint=endpoint

    def redirect_request(self,request,fp,code,msg,headers,newurl):
        if self.endpoint is None:
            return None
        target=urllib.parse.urlsplit(newurl)
        if code!=302 or request.get_method()!='GET' or (target.scheme,target.hostname,target.port)!=(self.endpoint.scheme,self.endpoint.hostname,self.endpoint.port) or target.username is not None or target.password is not None or target.fragment:
            raise RuntimeError('Redirect crossed its owned fixture boundary')
        parts=target.path.split('/')
        try:
            valid=len(parts)==4 and parts[:3]==['','benchmark-files','bench'] and str(uuid.UUID(parts[3]))==parts[3]
        except ValueError:
            valid=False
        query=urllib.parse.parse_qs(target.query)
        if not valid or query.get('X-Amz-Algorithm')!=['AWS4-HMAC-SHA256'] or len(query.get('X-Amz-Signature',[]))!=1:
            raise RuntimeError('Redirect is not an owned presigned object read')
        forwarded=super().redirect_request(request,fp,code,msg,headers,newurl)
        forwarded.remove_header('Authorization')
        forwarded.remove_header('Cookie')
        return forwarded


def opener(s3_endpoint=None):
    # Fixture traffic is loopback-only, including its real provider. A system
    # proxy must never receive credentials or signed object URLs from this test.
    return urllib.request.build_opener(urllib.request.ProxyHandler({}),FixtureRedirect(s3_endpoint))

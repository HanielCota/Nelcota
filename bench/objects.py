"""Bounded physical-store inventory for owned disk and real S3 fixtures."""
import datetime
import hashlib
import hmac
from pathlib import Path
import urllib.parse
import urllib.request
import uuid
import xml.etree.ElementTree as ET


class S3Inventory:
    def __init__(self, endpoint, access, secret, bucket):
        parsed = urllib.parse.urlsplit(endpoint)
        if parsed.scheme not in ('http','https') or parsed.hostname not in ('127.0.0.1','localhost') or parsed.path not in ('','/'):
            raise ValueError('Inventory is restricted to an owned loopback provider')
        self.endpoint = endpoint.rstrip('/')
        self.host = parsed.netloc
        self.access, self.secret = access, secret
        self.path = '/'+urllib.parse.quote(bucket,safe='')

    def query(self, parameters):
        now = datetime.datetime.now(datetime.timezone.utc)
        day, stamp = now.strftime('%Y%m%d'), now.strftime('%Y%m%dT%H%M%SZ')
        query = urllib.parse.urlencode(sorted(parameters.items()),quote_via=urllib.parse.quote)
        digest = hashlib.sha256(b'').hexdigest()
        headers = f'host:{self.host}\nx-amz-content-sha256:{digest}\nx-amz-date:{stamp}\n'
        signed = 'host;x-amz-content-sha256;x-amz-date'
        canonical = f'GET\n{self.path}\n{query}\n{headers}\n{signed}\n{digest}'
        scope = day+'/us-east-1/s3/aws4_request'
        message = f'AWS4-HMAC-SHA256\n{stamp}\n{scope}\n'+hashlib.sha256(canonical.encode()).hexdigest()
        key = ('AWS4'+self.secret).encode()
        for value in [day,'us-east-1','s3','aws4_request']:
            key = hmac.new(key,value.encode(),hashlib.sha256).digest()
        authorization = f'AWS4-HMAC-SHA256 Credential={self.access}/{scope}, SignedHeaders={signed}, Signature='+hmac.new(key,message.encode(),hashlib.sha256).hexdigest()
        request = urllib.request.Request(self.endpoint+self.path+'?'+query,headers={'Authorization':authorization,'x-amz-date':stamp,'x-amz-content-sha256':digest})
        with urllib.request.urlopen(request,timeout=10) as response:
            body = response.read(1024*1024+1)
        if len(body)>1024*1024:
            raise RuntimeError('Provider inventory response exceeded its byte budget')
        return ET.fromstring(body)

    def inventory(self):
        keys, total_bytes, continuation = [], 0, None
        for _ in range(20):
            parameters = {'list-type':'2','max-keys':'1000'}
            if continuation:
                parameters['continuation-token']=continuation
            root = self.query(parameters)
            for node in root.findall('{*}Contents'):
                key, size = node.findtext('{*}Key'), int(node.findtext('{*}Size','-1'))
                if not key or size < 0:
                    raise RuntimeError('Invalid provider object evidence')
                keys.append(key); total_bytes += size
            truncated = root.findtext('{*}IsTruncated')
            if truncated not in ('true','false'):
                raise RuntimeError('Missing provider truncation evidence')
            if truncated == 'false':
                break
            following = root.findtext('{*}NextContinuationToken')
            if not following or following == continuation:
                raise RuntimeError('Invalid provider continuation evidence')
            continuation = following
        if truncated != 'false':
            raise RuntimeError('Provider object inventory exceeded its page budget')
        uploads = self.query({'uploads':'','max-uploads':'1000'})
        if uploads.findtext('{*}IsTruncated') != 'false':
            raise RuntimeError('Provider multipart inventory exceeded its page budget')
        return {'keys':keys,'bytes':total_bytes,'multipart_uploads':len(uploads.findall('{*}Upload'))}


def disk_inventory(root):
    root = Path(root)
    keys, total_bytes = [], 0
    for path in root.rglob('*'):
        if path.is_symlink():
            raise RuntimeError('Unexpected symlink in the owned storage fixture')
        if path.is_file():
            keys.append(path.relative_to(root).as_posix())
            total_bytes += path.stat().st_size
        if len(keys)>20000:
            raise RuntimeError('Disk object inventory exceeded its evidence budget')
    return {'keys':keys,'bytes':total_bytes,'multipart_uploads':None}


def reconcile(inventory, known):
    if inventory['multipart_uploads'] not in (None,0):
        raise RuntimeError('Multipart state remained after traffic and cleanup grace')
    keys = set(inventory['keys'])
    if len(keys) != len(inventory['keys']):
        raise RuntimeError('Duplicate keys in provider inventory')
    unknown = sorted(keys-set(known))
    for key in unknown:
        parts = key.split('/')
        if len(parts) != 2:
            raise RuntimeError('Unpublished temporary storage state remained after traffic')
        try:
            version = uuid.UUID(parts[1])
        except ValueError:
            raise RuntimeError('Unpublished temporary storage key remained after traffic') from None
        if version.version != 7:
            raise RuntimeError('Unrecognized unpublished storage version')
    # The product permits recent unpublished versions for its 24-hour GC grace.
    # Preserve that distinction; recent candidates alone do not prove a leak.
    return {'physical_objects':len(keys),'physical_bytes':inventory['bytes'],'multipart_uploads':inventory['multipart_uploads'],'unreferenced_keys':unknown,'gc_grace_seconds':86400}

import socket, requests, requests.adapters as A
from requests.models import PreparedRequest
from unittest import mock
a=A.HTTPAdapter()
r=requests.Request('GET','https://a.example/').prepare()
p1=a._get_connection(r,True,cert=('a.crt','a.key'))
r2=requests.Request('GET','https://b.example/').prepare()
p2=a._get_connection(r2,True,cert=None)
print(p1.conn_kw.get('ssl_context') is p2.conn_kw.get('ssl_context') is A._preloaded_ssl_context, p1.conn_kw.get('cert_file'), p2.conn_kw.get('cert_file'))
# custom adapter ctx
import ssl
from urllib3.util.ssl_ import create_urllib3_context
class B(A.HTTPAdapter):
    def init_poolmanager(self,*a,**kw):
        self.ctx=create_urllib3_context(ciphers='ECDHE+AESGCM')
        super().init_poolmanager(*a,ssl_context=self.ctx,**kw)
b=B()
p=b._get_connection(r,True,cert=None)
print("custom kept:", p.conn_kw.get('ssl_context') is b.ctx)

import socket, urllib3.util.ssl_ as S
with mock.patch.object(ssl.SSLContext,'load_cert_chain') as m:
    pass
import os
d='/home/jack/.t3/bench-runs/2026-09-29-claude-ce-sonnet-5-5-high/att-043/clone/tests/certs/mtls/client'
print(os.listdir(d))
crt=os.path.join(d,'client.pem'); key=os.path.join(d,'client.key')
calls=[]
orig=ssl.SSLContext.load_cert_chain
class Ctx(ssl.SSLContext): pass
import urllib3.util.ssl_ as S
sock=socket.socketpair()[0]
try:
    with mock.patch.object(type(A._preloaded_ssl_context),'load_cert_chain',lambda self,*a,**k: calls.append((id(self),a))):
        try: S.ssl_wrap_socket(sock, certfile=crt, keyfile=key, ssl_context=p1.conn_kw['ssl_context'], server_hostname='a.example')
        except Exception as e: print('wrap err', type(e).__name__)
except Exception as e: print(e)
print(calls, id(A._preloaded_ssl_context))

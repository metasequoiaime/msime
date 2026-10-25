#!/usr/bin/env python3
"""供 Win7／现代 Windows 传输验收使用的合成 TLS WebSocket 服务，不接入真实语音账号。"""
import argparse, base64, hashlib, socketserver, ssl, struct
class Handler(socketserver.StreamRequestHandler):
 def read_frame(self):
  header=self.rfile.read(2)
  if len(header)!=2: return None,None
  opcode=header[0]&15; size=header[1]&127
  if size==126: size=struct.unpack('!H',self.rfile.read(2))[0]
  if size==127: size=struct.unpack('!Q',self.rfile.read(8))[0]
  if size>1048576: raise ValueError('synthetic frame too large')
  mask=self.rfile.read(4) if header[1]&128 else None
  data=self.rfile.read(size)
  if len(data)!=size: return None,None
  if mask: data=bytes(x^mask[i%4] for i,x in enumerate(data))
  return opcode,data
 def frame(self,opcode,data=b'',final=True):
  size=len(data); h=bytes([(128 if final else 0)|opcode])
  h+=bytes([size]) if size<126 else (bytes([126])+struct.pack('!H',size) if size<65536 else bytes([127])+struct.pack('!Q',size))
  self.wfile.write(h+data); self.wfile.flush()
 def handle(self):
  try:
   first=self.rfile.readline(4096).decode('ascii').split()
   if len(first)!=3: return
   path=first[1]; headers={}
   while True:
    line=self.rfile.readline(8192)
    if line==b'\r\n': break
    if not line: return
    k,v=line.decode('ascii').split(':',1); headers[k.lower()]=v.strip()
   if headers.get('x-msime-synthetic')!='allowed': return
   if path=='/redirect':
    self.wfile.write(b'HTTP/1.1 302 Found\r\nLocation: wss://127.0.0.1:9/forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n'); return
   accept=base64.b64encode(hashlib.sha1((headers['sec-websocket-key']+'258EAFA5-E914-47DA-95CA-C5AB0DC85B11').encode()).digest())
   self.wfile.write(b'HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: '+accept+b'\r\n\r\n'); self.wfile.flush()
   if path=='/oversize': self.frame(2,b'x'*1048577); return
   if path=='/text': self.frame(1,b'synthetic text'); return
   if path=='/close': self.frame(8,b'\x03\xe8'); return
   while True:
    opcode,data=self.read_frame()
    if opcode is None or opcode==8: return
    if opcode==10: continue
    if opcode!=2: raise ValueError('synthetic client must send binary')
    middle=len(data)//2
    self.frame(2,data[:middle],False)
    self.frame(9,b'synthetic ping')
    self.frame(0,data[middle:])
  except (OSError, ValueError, KeyError): pass
class Server(socketserver.ThreadingTCPServer):
 allow_reuse_address=True
 daemon_threads=True
 def handle_error(self,*args): pass
def main():
 parser=argparse.ArgumentParser(description=__doc__)
 parser.add_argument('--bind',default='127.0.0.1')
 parser.add_argument('--port',type=int,default=18778)
 parser.add_argument('--certificate',required=True)
 parser.add_argument('--key',required=True)
 args=parser.parse_args()
 ctx=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
 ctx.minimum_version=ssl.TLSVersion.TLSv1_2
 ctx.load_cert_chain(args.certificate,args.key)
 with Server((args.bind,args.port),Handler) as server:
  server.socket=ctx.wrap_socket(server.socket,server_side=True)
  server.serve_forever()
if __name__=='__main__': main()

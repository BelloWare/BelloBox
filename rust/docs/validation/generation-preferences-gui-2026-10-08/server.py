from http.server import BaseHTTPRequestHandler,HTTPServer
from pathlib import Path
import json,time
D=Path('/workspace/shared/generation-gui-r1')
class H(BaseHTTPRequestHandler):
 def log_message(self,*a): pass
 def do_GET(self):
  with (D/'requests.jsonl').open('a') as f: f.write(json.dumps({'method':'GET','path':self.path})+'\n')
  b=json.dumps({'data':[{'id':'gui-selected-model'},{'id':'gui-other-model'}]}).encode();self.send_response(200);self.send_header('Content-Length',str(len(b)));self.end_headers();self.wfile.write(b)
 def do_POST(self):
  body=json.loads(self.rfile.read(int(self.headers.get('Content-Length',0))))
  with (D/'requests.jsonl').open('a') as f:f.write(json.dumps({'method':'POST','path':self.path,'body':body})+'\n')
  if (D/'hold').exists():
   (D/'admitted-held').write_text('true')
   until=time.monotonic()+90
   while (D/'hold').exists() and time.monotonic()<until:time.sleep(.05)
  b=b'data: {"choices":[{"delta":{"content":"Hello from GUI fixture"}}]}\n\ndata: [DONE]\n\n';self.send_response(200);self.send_header('Content-Length',str(len(b)));self.end_headers();self.wfile.write(b)
s=HTTPServer(('127.0.0.1',0),H)
(D/'endpoint').write_text(f'http://127.0.0.1:{s.server_port}/v1')
(D/'config').mkdir(exist_ok=True)
(D/'config/settings.json').write_text(json.dumps({'provider_kind':'openai','provider_endpoint':f'http://127.0.0.1:{s.server_port}/v1','provider_model':'gui-selected-model','appearance':'Light'}))
s.serve_forever()

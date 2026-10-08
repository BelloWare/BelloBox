from http.server import BaseHTTPRequestHandler,HTTPServer
from pathlib import Path
import json,time
D=Path('/workspace/shared/clock-gui-r2')
class H(BaseHTTPRequestHandler):
 def log_message(self,*a): pass
 def do_POST(self):
  body=json.loads(self.rfile.read(int(self.headers.get('Content-Length',0))))
  with (D/'requests.jsonl').open('a') as f:f.write(json.dumps({'method':'POST','path':self.path,'body':body})+'\n')
  if (D/'hold').exists():
   (D/'admitted-held').write_text('true')
   until=time.monotonic()+90
   while (D/'hold').exists() and time.monotonic()<until:time.sleep(.05)
  content=json.dumps({'answer':'Tokyo is nine hours ahead of UTC.','suggestion':{'timeZoneIDs':['Asia/Tokyo'],'replaceLocations':False}})
  b=('data: '+json.dumps({'choices':[{'delta':{'content':content}}]})+'\n\ndata: [DONE]\n\n').encode()
  self.send_response(200);self.send_header('Content-Type','text/event-stream');self.send_header('Content-Length',str(len(b)));self.end_headers();self.wfile.write(b)
s=HTTPServer(('127.0.0.1',0),H)
(D/'endpoint').write_text(f'http://127.0.0.1:{s.server_port}/v1')
(D/'config').mkdir(exist_ok=True)
p=D/'config/settings.json'
settings=json.loads(p.read_text()) if p.exists() else {'appearance':'Light'}
settings.update({'provider_kind':'openai','provider_endpoint':f'http://127.0.0.1:{s.server_port}/v1','provider_model':'clock-gui-model'})
p.write_text(json.dumps(settings))
s.serve_forever()

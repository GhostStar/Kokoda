import argparse,base64,json,subprocess,time
from pathlib import Path
parser=argparse.ArgumentParser()
parser.add_argument('--client',required=True)
parser.add_argument('--codec',choices=['images','h264'],default='images')
parser.add_argument('--address',required=True)
parser.add_argument('--pairing',required=True)
parser.add_argument('--result-file',required=True,help='Local path to the shared fixture result file')
parser.add_argument('--push',action='store_true')
args=parser.parse_args()
frames={}
frame_counts={}
p=subprocess.Popen([args.client,args.address,args.pairing,'--stream-bridge' if args.push else '--bridge'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
def receive():
 line=p.stdout.readline()
 if not line:raise RuntimeError(p.stderr.read())
 value=json.loads(line)
 if value.get('event'):
  assert value['type']=='frame',value
  key=(value['id'],value['token'])
  frames[key]=value
  frame_counts[key]=frame_counts.get(key,0)+1
 return value
def exchange(obj,allow_error=False):
 p.stdin.write(json.dumps(obj)+'\n');p.stdin.flush()
 value=receive()
 while value.get('event'):value=receive()
 if value.get('type')=='error' and not allow_error:raise RuntimeError(value)
 return value
def group():return exchange({'command':'window_group'})['windows']
def frame(w):
 if args.push:
  key=(w['id'],w['token'])
  while key not in frames:receive()
  return frames[key]
 for _ in range(40):
  result=exchange({'command':'frame','id':w['id'],'token':w['token'],'after':0})
  if result['type']=='frame':return result
  time.sleep(.05)
 raise RuntimeError('no frame')
def click(w,x,y):
 f=frame(w);point=dict(x=x,y=y,width=f['width'],height=f['height'],button=0)
 exchange({'command':'input','id':w['id'],'token':w['token'],'events':[dict(kind='button',down=True,**point),dict(kind='button',down=False,**point)]})
 time.sleep(.2)
try:
 if args.push:exchange({'command':'start_stream'})
 roots=exchange({'command':'list_windows'})['windows'];root=next(w for w in roots if w['title']=='Kokoda Group Test')
 exchange({'command':'select_window','id':root['id'],'scope':'application','codec':args.codec})
 root=next(w for w in group() if w['id']==root['id'])
 click(root,.18,.28)
 members=group();card=next(w for w in members if w['title']=='Kokoda Group Card')
 assert card['owner'] is None,card
 assert card['process_id']==root['process_id']
 f=frame(card);assert f['format']=='png' and base64.b64decode(f['image']).startswith(b'\x89PNG')
 click(card,.5,.5)
 exchange({'command':'input','id':card['id'],'token':card['token'],'events':[{'kind':'text','text':'group 中文 456'}]})
 time.sleep(.2)
 assert Path(args.result_file).read_text()=='group 中文 456'
 print('PASS: ownerless card discovery, PNG frame and routed Unicode input',flush=True)
 if args.push:
  time.sleep(.5);exchange({'command':'ping'})
  key=(card['id'],card['token']);before=frame_counts.get(key,0)
  for _ in range(8):
   time.sleep(.1);exchange({'command':'ping'})
  # Caret blinking may still change pixels; unchanged captures must not stream at 15 FPS.
  sent=frame_counts.get(key,0)-before
  assert sent<=5, ('Static card repeatedly retransmitted',sent)
  print(f'PASS: static card sent only {sent} changed frames during idle sampling',flush=True)

 click(root,.55,.28)
 assert not any(w['id']==card['id'] for w in group())
 click(root,.18,.28)
 newcard=next(w for w in group() if w['title']=='Kokoda Group Card')
 assert newcard['token']!=card['token']
 stale=exchange({'command':'input','id':card['id'],'token':card['token'],'events':[{'kind':'release'}]},True)
 assert stale['type']=='error' and not stale['selection_lost']
 assert exchange({'command':'ping'})['type']=='pong'
 frame(newcard)
 print('PASS: hide removes card; re-show changes token; stale input rejected without disconnect',flush=True)
 click(root,.18,.63)
 assert not any(w['id']==newcard['id'] for w in group())
 frame(root)
 outsider=exchange({'command':'input','id':1,'token':1,'events':[{'kind':'release'}]},True)
 assert outsider['type']=='error' and not outsider['selection_lost']
 print('PASS: destroy removes card, root survives, out-of-group input rejected',flush=True)
 # A disabled owner must not steal the modal's focus on passive motion.
 click(root,.85,.28)
 modal=next(w for w in group() if w['title']=='Kokoda Modal Dialog')
 assert exchange({'command':'window_group'})['foreground']==modal['id']
 rf=frame(root)
 exchange({'command':'input','id':root['id'],'token':root['token'],'events':[
  dict(kind='move',x=.5,y=.5,width=rf['width'],height=rf['height']) for _ in range(20)]})
 assert exchange({'command':'window_group'})['foreground']==modal['id'], 'Hover stole modal focus'
 assert exchange({'command':'ping'})['type']=='pong'
 exchange({'command':'input','id':modal['id'],'token':modal['token'],'events':[
  dict(kind='key',vk=27,down=True),dict(kind='key',vk=27,down=False)]})
 time.sleep(.2)
 assert not any(w['id']==modal['id'] for w in group())
 print('PASS: passive owner motion preserves modal focus; input and connection remain responsive',flush=True)
 exchange({'command':'close_window'})
 exchange({'command':'select_window','id':root['id'],'scope':'window','codec':args.codec})
 root=group()[0];click(root,.18,.28)
 assert len(group())==1
 print('PASS: single-window mode stays single',flush=True)
finally:
 p.stdin.close();p.wait(timeout=20)

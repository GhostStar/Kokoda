#!/usr/bin/env python3
"""Two independent synthetic applications over one TLS connection."""
import argparse,json,subprocess,time
from pathlib import Path
p=argparse.ArgumentParser()
for name in ['client','address','pairing','result_a','result_b']:p.add_argument('--'+name.replace('_','-'),required=True)
p.add_argument('--push',action='store_true')
a=p.parse_args()
class Bridge:
 def __init__(self):
  self.frames={}
  self.p=subprocess.Popen([a.client,a.address,a.pairing,'--stream-bridge' if a.push else '--bridge'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  if a.push:self.call(0,'start_stream')
 def read(self):
  line=self.p.stdout.readline()
  if not line:raise RuntimeError(self.p.stderr.read())
  r=json.loads(line)
  if r.get('event'):
   assert r['type']=='frame',r
   self.frames[r['session_id']]=r
  return r
 def call(self,sid,command,**payload):
  self.p.stdin.write(json.dumps(dict(command=command,session_id=sid,**payload))+'\n');self.p.stdin.flush()
  while True:
   r=self.read()
   if not r.get('event'):break
  assert r.get('session_id')==sid,r.get('type');return r
 def close(self):
  self.p.terminate()
  try:self.p.wait(timeout=5)
  except subprocess.TimeoutExpired:self.p.kill();self.p.wait()
b=Bridge();roots={};tokens={};tickets={}
def ok(sid,command,**kw):
 r=b.call(sid,command,**kw);assert r['type']!='error',r;return r
def events(sid,items):return ok(sid,'input',id=roots[sid],token=tokens[sid],events=items)
def key(vk):return [{'kind':'key','vk':vk,'down':True},{'kind':'key','vk':vk,'down':False}]
def click(sid):
 bounds=ok(sid,'window_group')['root_bounds'];w=bounds['right']-bounds['left'];h=bounds['bottom']-bounds['top']
 events(sid,[dict(kind='button',x=.5,y=.4,width=w,height=h,button=0,down=True),dict(kind='button',x=.5,y=.4,width=w,height=h,button=0,down=False)])
def text(path):time.sleep(.15);return Path(path).read_text(encoding='utf-8-sig')
def frame(sid):
 if a.push:
  while sid not in b.frames:b.read()
  return b.frames.pop(sid)
 for _ in range(30):
  r=ok(sid,'frame',id=roots[sid],token=tokens[sid],after=0)
  if r['type']=='frame':return r
  time.sleep(.05)
 raise AssertionError(f'No new frame for session {sid}: {r}')
try:
 listed=ok(0,'list_windows');assert listed['multi_session']
 for sid,title in [(1,'Kokoda Multi A Test'),(2,'Kokoda Multi B Test')]:
  roots[sid]=next(w['id'] for w in listed['windows'] if w['title']==title)
  r=ok(sid,'select_window',id=roots[sid],scope='application',codec='h264');tickets[sid]=r['resume_token']
  group=ok(sid,'window_group');tokens[sid]=next(w['token'] for w in group['windows'] if w['id']==roots[sid])
 for sid in [1,2]:
  assert frame(sid)['id']==roots[sid]
  print(f'PASS: session {sid} first frame',flush=True)
 click(1);events(1,[dict(kind='key',vk=17,down=True)]+key(65)+[dict(kind='key',vk=17,down=False)]+[dict(kind='text',text='Kokoda multi A 中文')])
 assert text(a.result_a)=='Kokoda multi A 中文'
 click(2);events(2,[dict(kind='key',vk=17,down=True)]+key(65)+[dict(kind='key',vk=17,down=False)]+[dict(kind='text',text='Kokoda multi B 中文')])
 assert text(a.result_a)=='Kokoda multi A 中文'
 assert text(a.result_b)=='Kokoda multi B 中文'
 events(1,[dict(kind='key',vk=16,down=True)])
 click(2);events(2,key(35)+key(65))
 assert text(a.result_b).endswith('a'),'Shift from first app leaked into second app'
 assert text(a.result_a)=='Kokoda multi A 中文'
 events(2,[dict(kind='key',vk=16,down=True)])
 events(1,[dict(kind='release')])
 events(2,key(65)+[dict(kind='release')])
 assert text(a.result_b).endswith('aA'),'Late release from first app released second app Shift'
 wrong=b.call(1,'input',id=roots[2],token=tokens[2],events=[dict(kind='text',text='must not arrive')]);assert wrong['type']=='error'
 assert frame(2)['type']=='frame'
 duplicate=b.call(3,'select_window',id=roots[1],scope='application');assert duplicate['type']=='error'
 click(1);events(1,key(35));assert frame(1)['type']=='frame'
 print('PASS: simultaneous frames, independent text input, cross-app key release, wrong-target and duplicate-session rejection',flush=True)
 b.close();b=Bridge()
 for sid in [1,2]:
  restored=ok(sid,'resume_window',resume_token=tickets[sid]);assert restored['id']==roots[sid]
  tickets[sid]=restored['resume_token'];g=ok(sid,'window_group');tokens[sid]=next(w['token'] for w in g['windows'] if w['id']==roots[sid])
  frame(sid)
 ok(1,'close_window')
 assert b.call(1,'frame',id=roots[1],token=tokens[1],after=0)['type']=='error'
 click(2);events(2,key(35));assert frame(2)['type']=='frame'
 events(2,[dict(kind='text',text=' still live')])
 assert text(a.result_b).endswith(' still live')
 assert b.call(1,'resume_window',resume_token=tickets[1])['type']=='error'
 ok(2,'close_window')
 print('PASS: both applications restore on one connection; closing one leaves the other interactive',flush=True)
finally:b.close()

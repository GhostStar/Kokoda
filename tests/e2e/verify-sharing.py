#!/usr/bin/env python3
"""Restricted Windows sharing against the named local fixture; never user windows."""
import argparse,json,subprocess,time
from pathlib import Path
p=argparse.ArgumentParser()
for name in ['client','address','pairing','state','commands']:p.add_argument('--'+name,required=True)
p.add_argument('--empty',action='store_true')
p.add_argument('--push',action='store_true')
a=p.parse_args()
child=subprocess.Popen([a.client,a.address,a.pairing,'--stream-bridge' if a.push else '--bridge'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
permitted=set();frames=[]
def call(command,**fields):
 child.stdin.write(json.dumps(dict(command=command,session_id=1,**fields))+'\n');child.stdin.flush()
 while True:
  line=child.stdout.readline()
  if not line:raise RuntimeError('Bridge ended before reply')
  value=json.loads(line)
  assert value['type']!='bridge_error',value.get('category')
  if value.get('event'):
   if value['type']=='error':
    assert value.get('selection_lost') is True,'Unexpected stream error'
    permitted.clear();frames.clear();continue
   assert value['type']=='frame' and value['id'] in permitted,'Stream exposed an unauthorized target'
   frames[:]=[value]
  else:return value
def change(command):
 Path(a.commands).write_text(command)
 for _ in range(100):
  try:
   if json.loads(Path(a.state).read_text())['command']==command:return
  except (OSError,ValueError):pass
  time.sleep(.05)
 raise AssertionError('Fixture did not observe command')
try:
 for _ in range(100):
  try:state=json.loads(Path(a.state).read_text());break
  except (OSError,ValueError):time.sleep(.1)
 else:raise AssertionError('Fixture did not become ready')
 permitted={state['root'],state['dialog']}
 if a.push:assert call('start_stream')['type']=='stream_started'
 listing=call('list_windows');assert listing['read_only'] is True
 ids={w['id'] for w in listing['windows']}
 assert ids==(set() if a.empty else {state['root'],state['dialog']}), 'Catalog exposed an unauthorized window'
 for command in ['input','clipboard','clipboard_poll','window_action']:
  response=call(command,id=state['root'],token=1,events=[{'kind':'text','text':'must not arrive'}],action='copy')
  assert response['code']=='sharing_read_only'
 assert call('select_window',id=state['sibling'])['type']=='error'
 if a.empty:
  assert call('select_window',id=state['root'])['type']=='error'
  print('PASS: default-empty catalog, guessed IDs and all input/clipboard requests denied')
 else:
  selected=call('select_window',id=state['root'],scope='application',codec='h264')
  assert selected['type']=='selected' and selected['read_only']
  ticket=selected['resume_token'];group=call('window_group')
  assert {w['id'] for w in group['windows']}==ids
  assert group['foreground'] is None or group['foreground'] in ids
  token=next(w['token'] for w in group['windows'] if w['id']==state['root'])
  for _ in range(100):
   response=call('ping') if a.push else call('frame',id=state['root'],token=token,after=0)
   frame=(frames[-1] if frames else response) if a.push else response
   if frame['type']=='frame':break
   time.sleep(.05)
  assert frame['type']=='frame'
  assert call('frame',id=state['sibling'],token=token)['type']=='error'
  change('detach');group=call('window_group');assert {w['id'] for w in group['windows']}=={state['root']}
  permitted={state['root']}
  change('revoke')
  assert call('list_windows')['windows']==[]
  rejected=call('frame',id=state['root'],token=token);assert rejected['type']=='error' and 'image' not in rejected
  assert call('window_group')['type']=='error'
  permitted=set();time.sleep(.2);assert call('ping')['type']=='pong'
  change('restore')
  assert call('resume_window',resume_token=ticket)['type']=='error'
  permitted={state['root']}
  assert call('select_window',id=state['root'])['type']=='selected'
  assert call('close_window')['type']=='closed'
  print('PASS: owned dialog only, sibling/guessed capture denied, read-only gates, owner/title revocation, fresh selection required')
finally:
 child.stdin.close()
 try:child.wait(timeout=20)
 except subprocess.TimeoutExpired:child.kill();child.wait()

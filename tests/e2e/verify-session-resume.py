#!/usr/bin/env python3
"""Reconnect to an unchanged test window; never replay input or clipboard commands."""
import argparse, json, subprocess, time
p=argparse.ArgumentParser()
for name in ['client','address','pairing','window']:p.add_argument('--'+name,required=True)
a=p.parse_args()
class Bridge:
 def __init__(self):
  self.p=subprocess.Popen([a.client,a.address,a.pairing,'--bridge'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
 def call(self,**request):
  self.p.stdin.write(json.dumps(request)+'\n');self.p.stdin.flush()
  line=self.p.stdout.readline()
  if not line:raise RuntimeError(self.p.stderr.read())
  return json.loads(line)
 def stop(self):
  if self.p.poll() is None:self.p.terminate()
  try:self.p.wait(timeout=5)
  except subprocess.TimeoutExpired:self.p.kill();self.p.wait()
b=Bridge()
try:
 windows=b.call(command='list_windows')['windows']
 target=next(w for w in windows if w['title']==a.window)
 selected=b.call(command='select_window',id=target['id'],scope='application',codec='h264')
 assert selected['type']=='selected',selected
 ticket=selected['resume_token'];b.stop();b=Bridge()
 assert b.call(command='list_windows')['type']=='windows'
 rejected=b.call(command='resume_window',resume_token='invalid')
 assert rejected['type']=='error' and rejected['selection_lost']
 restored=b.call(command='resume_window',resume_token=ticket)
 assert restored['type']=='selected' and restored['id']==target['id']
 assert restored['resume_token']!=ticket
 assert b.call(command='window_group')['type']=='window_group'
 for _ in range(50):
  frame=b.call(command='frame',after=0)
  if frame['type']=='frame':break
  time.sleep(.05)
 assert frame['type']=='frame'
 ping=b.call(command='ping');assert ping['type']=='pong' and ping['round_trip_ms']>=0
 latest=restored['resume_token']
 assert b.call(command='close_window')['type']=='closed'
 assert b.call(command='resume_window',resume_token=latest)['type']=='error'
 assert b.call(command='ping')['type']=='pong'
 print('PASS: lost transport restores same window and frames; invalid/closed tickets rejected; heartbeat timing available')
finally:b.stop()

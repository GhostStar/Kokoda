#!/usr/bin/env python3
"""Synthetic playback-credit and adaptive-rate integration; no personal windows."""
import argparse, collections, json, queue, statistics, subprocess, threading, time
from pathlib import Path
p=argparse.ArgumentParser()
for name in ('client','address','secret'):p.add_argument('--'+name,required=True)
a=p.parse_args()
bridge=subprocess.Popen([a.client,a.address,'--password-stdin','--stream-bridge'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
lines=queue.Queue(maxsize=64)
def reader():
    for line in bridge.stdout:lines.put(json.loads(line))
    lines.put({'type':'eof'})
threading.Thread(target=reader,daemon=True).start()
bridge.stdin.write(Path(a.secret).read_text()+'\n');bridge.stdin.flush()
paused=None; held=[]; counts=collections.Counter(); rates={}; ceilings={}; reasons={}; slow=None; queued=None
latencies=[]
def send(command,**fields):
    bridge.stdin.write(json.dumps(dict(command=command,session_id=1,**fields))+'\n');bridge.stdin.flush()
def feedback(v):
    send('video_feedback',bridge_async=True,id=v['id'],token=v['token'],stream_epoch=v['stream_epoch'],sequence=v['sequence'],decode_ms=80 if v['id']==slow else 2,queue_ms=160 if v['id']==queued else 0)
def receive():
    v=lines.get(timeout=20)
    assert v['type'] not in ('error','bridge_error','eof'),v.get('message',v['type'])
    if v['type']=='frame':
        counts[v['id']]+=1
        if v['id']==paused:held.append(v)
        else:feedback(v)
    if v['type']=='video_feedback_ok':
        rates[v['id']]=(v.get('fps'),v.get('bitrate'));ceilings[v['id']]=v.get('bitrate_ceiling');reasons[v['id']]=v.get('adaptation')
    return v
def call(command,**fields):
    started=time.monotonic();send(command,**fields)
    while True:
        v=receive()
        if not v.get('event') and not v.get('bridge_async'):
            if command=='ping':latencies.append(1000*(time.monotonic()-started))
            return v
def sample(seconds):
    deadline=time.monotonic()+seconds
    while time.monotonic()<deadline:
        call('ping');time.sleep(.015)
try:
    catalog=call('list_windows');assert catalog['playback_feedback'] and catalog['bridge_pipeline']
    deadline=time.monotonic()+15
    root=None
    while root is None:
        root=next((w['id'] for w in catalog['windows'] if w['title']=='Kokoda Popup Video Root'),None)
        if root is not None:break
        assert time.monotonic()<deadline,'Synthetic animation fixture did not start'
        time.sleep(.2);catalog=call('list_windows')
    call('start_stream')
    call('select_window',id=root,scope='application',codec='h264',fps=30,bitrate=2000000,max_width=640,encoder='software',alpha_video=True,playback_feedback=True,window_management=True)
    members=call('window_group')['windows']
    popup=next(w for w in members if w['title']=='Kokoda Popup Video Opaque')
    other=next(w for w in members if w['title']=='Kokoda Popup Video Independent')
    paused=popup['id'];sample(3)
    assert len(held)==2,('Credit limit failed',len(held))
    assert counts[other['id']]>=10,'Paused decoder blocked another window'
    print('PASS: paused window stopped at 2 frames; independent window received',counts[other['id']],'frames; control ping median/max ms',round(statistics.median(latencies),2),round(max(latencies),2),flush=True)
    paused=None
    for v in held:feedback(v)
    held.clear();before=counts[popup['id']];sample(2)
    assert counts[popup['id']]>before+5,'Consumption did not restart capture'
    # Deliberately injected decoder telemetry exercises the actual host controller;
    # these values are not a measured hardware decode performance claim.
    slow=popup['id'];sample(6)
    reduced=rates[slow][0];assert reduced<30,('No adaptive reduction',rates[slow])
    assert rates[slow][1]==2000000,'Slow decoder blurred the stream'
    slow=None;sample(13)
    recovered=rates[popup['id']][0];assert reduced<recovered<=30,('No bounded recovery',reduced,recovered)
    queued=popup['id'];sample(9)
    assert rates[queued][0]<recovered and rates[queued][1]==2000000,('Client queue reduced image quality',rates[queued])
    assert reasons[queued]=='client_busy'
    queued=None
    print('PASS: sustained local decoder/queue pressure reduces FPS while preserving 2 Mbps, then recovers',flush=True)
    token=next(w['token'] for w in members if w['id']==root)
    call('video_settings',id=root,token=token,codec='h264',fps=30,bitrate=2000000,max_width=640,encoder='software',resolution_bitrate=True);sample(2)
    sibling_budget=ceilings[other['id']]
    call('window_action',id=popup['id'],token=popup['token'],action='resize',width=1280,height=720,revision=1,final=True)
    call('video_viewport',id=popup['id'],token=popup['token'],width=1280,height=720);sample(4)
    assert ceilings[popup['id']]==8000000 and ceilings[other['id']]==sibling_budget,('Per-window Retina budget',ceilings)
    print('PASS: larger popup gets 8 Mbps pixel budget while sibling retains',sibling_budget/1000000,'Mbps',flush=True)
    call('video_settings',id=root,token=token,codec='h264',fps=15,bitrate=1000000,max_width=640,encoder='software');sample(2)
    assert all(fps is None or fps<=15 for fps,_ in rates.values())
    call('close_window')
    print('PASS: released credits resume; injected slow decoder reduced FPS',reduced,'then recovered to',recovered,'; live preset reset stays under new ceiling',flush=True)
finally:
    bridge.terminate()
    try:bridge.wait(timeout=5)
    except subprocess.TimeoutExpired:bridge.kill();bridge.wait()

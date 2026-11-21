#!/usr/bin/env python3
"""Isolated popup animation test. Never selects or prints personal windows."""
import argparse
import base64
import json
import queue
import statistics
import subprocess
import threading
import time
from pathlib import Path

p = argparse.ArgumentParser()
for name in ('client', 'address', 'secret', 'output'):
    p.add_argument('--' + name, required=True)
a = p.parse_args()
output = Path(a.output)
output.mkdir(parents=True, exist_ok=True)
process = subprocess.Popen([a.client, a.address, '--password-stdin', '--stream-bridge'],
                           stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
lines = queue.Queue(maxsize=32)
def reader():
    for line in process.stdout:
        lines.put(json.loads(line))
    lines.put({'type': 'bridge_eof'})
threading.Thread(target=reader, daemon=True).start()
process.stdin.write(Path(a.secret).read_text() + '\n')
process.stdin.flush()
frames, latencies = [], []
targets = {}
record = False
files = {}
def receive():
    value = lines.get(timeout=20)
    assert value['type'] not in ('error', 'bridge_error', 'bridge_eof'), value['type']
    if value['type'] == 'frame' and value.get('event'):
        if value['id'] in targets:
            frames.append((time.monotonic(), value))
            if record and value['format'] == 'h264':
                files[targets[value['id']]].write(json.dumps(value) + '\n')
    return value
def call(command, sid=1, **fields):
    start = time.monotonic()
    process.stdin.write(json.dumps(dict(command=command, session_id=sid, **fields)) + '\n')
    process.stdin.flush()
    while True:
        value = receive()
        if not value.get('event'):
            if command == 'ping': latencies.append(1000*(time.monotonic()-start))
            return value
def sample(seconds=5):
    start = time.monotonic()
    while time.monotonic()-start < seconds:
        receive()
        if not latencies or time.monotonic()-sample.last_ping > .25:
            call('ping', 0); sample.last_ping=time.monotonic()
    return [v for t,v in frames if t >= start], time.monotonic()-start
sample.last_ping = 0
def report(label, values, elapsed):
    result = {}
    for ident, name in targets.items():
        packets = [v for v in values if v['id'] == ident]
        assert packets, 'Synthetic animation stalled'
        result[name] = {'fps':round(len(packets)/elapsed,2), 'formats':sorted({v['format'] for v in packets}),
                        'mbps':round(sum(len(base64.b64decode(v['image'])) + len(base64.b64decode(v.get('alpha_png') or '')) for v in packets)*8/elapsed/1e6,2)}
    print(label, json.dumps(result), flush=True)
try:
    catalog = call('list_windows', 0)
    assert catalog.get('window_video_alpha')
    root = next(w['id'] for w in catalog['windows'] if w['title']=='Kokoda Popup Video Root')
    call('start_stream', 0)
    for enabled in (False, True):
        call('select_window', id=root, scope='application', codec='h264', alpha_video=enabled,
             fps=30, bitrate=2000000, max_width=640, encoder='software')
        members = call('window_group')['windows']
        targets = {w['id']:w['title'].split()[-1].lower() for w in members if w['title'].startswith('Kokoda Popup Video ') and w['id']!=root}
        assert set(targets.values()) == {'opaque','alpha','independent'}
        if enabled:
            files = {name:(output/(name+'.jsonl')).open('w') for name in targets.values()}
            record = True
        packets, elapsed = sample()
        assert all(v['format'] == ('h264' if enabled else 'png') for v in packets)
        report('video' if enabled else 'legacy PNG', packets, elapsed)
        if not enabled: call('close_window')
    root_token = next(w['token'] for w in members if w['id']==root)
    popup = next(w for w in members if targets.get(w['id'])=='opaque')
    call('video_viewport',id=popup['id'],token=popup['token'],width=320,height=200)
    packets,_ = sample(2)
    assert any(v['id']==popup['id'] and v['encoded_width']<=320 for v in packets)
    assert any(v['id']!=popup['id'] and v['encoded_width']>320 for v in packets)
    prior={v['id']:v['stream_epoch'] for v in packets}
    call('video_settings',id=root,token=root_token,codec='h264',fps=15,bitrate=1000000,max_width=640,encoder='software')
    packets,_ = sample(2)
    assert all(any(v['id']==ident and v['stream_epoch']>prior[ident] for v in packets) for ident in targets)
    record=False
    call('video_settings',id=root,token=root_token,codec='images')
    packets,_ = sample(2)
    assert all(any(v['id']==ident and v['format']=='png' for v in packets) for ident in targets)
    call('video_settings',id=root,token=root_token,codec='h264',fps=30,bitrate=2000000,max_width=640,encoder='software')
    packets,_ = sample(2)
    assert all(any(v['id']==ident and v['format']=='h264' for v in packets) for ident in targets)
    call('close_window')
    print('PASS: ordinary, layered and independent popups; independent viewport; live FPS/codec propagation and epoch resets',flush=True)
    print('Control ping median/max (ms):',round(statistics.median(latencies),2),round(max(latencies),2),flush=True)
finally:
    for stream in files.values(): stream.close()
    process.terminate()
    try: process.wait(timeout=5)
    except subprocess.TimeoutExpired: process.kill(); process.wait()

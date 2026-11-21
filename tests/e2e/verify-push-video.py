#!/usr/bin/env python3
"""Verify unsolicited video, control responsiveness, viewport changes and resume.
Only selects the synthetic animation fixture; does not send user input.
"""
import argparse
import json
import queue
import statistics
import subprocess
import threading
import time
from pathlib import Path

p = argparse.ArgumentParser()
for name in ('client', 'address', 'pairing', 'output'):
    p.add_argument('--' + name, required=True)
p.add_argument('--switch-settings', action='store_true')
p.add_argument('--seconds', type=float, default=8)
p.add_argument('--encoder', choices=['auto', 'software'], default='auto')
p.add_argument('--fps', type=int, default=30)
p.add_argument('--bitrate', type=int, default=2000000)
p.add_argument('--viewport', default='1280x720')
a = p.parse_args()
frames = []
latencies = []
last_sequence = 0
output = Path(a.output).open('w')


class Bridge:
    def __init__(self):
        self.process = subprocess.Popen(
            [a.client, a.address, a.pairing, '--stream-bridge'],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        self.lines = queue.Queue(maxsize=32)
        def read():
            for line in self.process.stdout:
                self.lines.put(json.loads(line))
            self.lines.put({'type': 'bridge_eof'})
        threading.Thread(target=read, daemon=True).start()

    def receive(self):
        global last_sequence
        response = self.lines.get(timeout=15)
        assert response['type'] not in ('error', 'bridge_error', 'bridge_eof'), response
        if response.get('event') and response['type'] == 'frame':
            assert response['session_id'] == 1
            assert response['sequence'] == last_sequence + 1, 'Encoded reference frame lost'
            last_sequence = response['sequence']
            frames.append((time.monotonic(), response['encode_ms'], response['encoder']))
            if response['format']=='h264': output.write(json.dumps(response) + '\n')
        return response

    def call(self, command, sid=1, **values):
        start = time.monotonic()
        self.process.stdin.write(json.dumps(dict(command=command, session_id=sid, **values)) + '\n')
        self.process.stdin.flush()
        while True:
            response = self.receive()
            if not response.get('event'):
                assert response.get('session_id', 0) == sid
                if command == 'ping': latencies.append((time.monotonic() - start) * 1000)
                return response

    def close(self):
        self.process.terminate()
        try: self.process.wait(timeout=5)
        except subprocess.TimeoutExpired: self.process.kill(); self.process.wait()


b = Bridge()
try:
    windows = b.call('list_windows', 0)
    assert windows['push_video']
    root = next(w['id'] for w in windows['windows'] if w['title'] == 'Kokoda Video Test')
    assert b.call('start_stream', 0)['type'] == 'stream_started'
    selected = b.call('select_window', id=root, scope='application', codec='h264', fps=a.fps,
                      bitrate=a.bitrate, max_width=1280, encoder=a.encoder)
    ticket = selected['resume_token']
    group = b.call('window_group')
    token = next(w['token'] for w in group['windows'] if w['id'] == root)
    assert b.call('frame',id=root,token=token,after=0)['type']=='stream_started'
    print('PASS: stale pull request resynchronizes push mode without consuming a video frame',flush=True)
    w, h = map(int, a.viewport.split('x'))
    b.call('video_viewport', id=root, token=token, width=w, height=h)
    start = time.monotonic()
    last_ping = start
    while time.monotonic() - start < a.seconds:
        b.receive()
        if time.monotonic() - last_ping >= .25:
            b.call('ping', 0)
            last_ping = time.monotonic()
    assert len(frames) >= 10
    elapsed = time.monotonic() - start
    print(json.dumps(dict(frames=len(frames), fps=round(len(frames)/elapsed, 2),
        encoder=sorted(set(f[2] for f in frames)),
        encode_ms_median=round(statistics.median(f[1] for f in frames), 2),
        ping_ms_median=round(statistics.median(latencies), 2), ping_ms_max=round(max(latencies), 2))), flush=True)
    if a.switch_settings:
        assert windows['live_video_settings']
        original_bounds=group['root_bounds']
        previous_epoch=0
        for codec,fps,bitrate in [('h264',60,8000000),('h264',30,2000000),('images',30,2000000),('h264',60,8000000)]:
            result=b.call('video_settings',id=root,token=token,codec=codec,fps=fps,bitrate=bitrate,max_width=1280)
            epoch=result['stream_epoch']
            assert epoch>previous_epoch
            previous_epoch=epoch
            for _ in range(100):
                response=b.receive()
                if response['type']=='frame' and response['stream_epoch']==epoch:
                    assert response['format']==('h264' if codec=='h264' else 'jpeg')
                    break
            else:raise AssertionError('New video settings did not produce a frame')
            current=b.call('window_group')
            assert current['root_bounds']==original_bounds
            assert next(w['token'] for w in current['windows'] if w['id']==root)==token
        print('PASS: live FPS/bitrate/codec switching preserves window identity and geometry',flush=True)
    b.call('video_viewport', id=root, token=token, width=960, height=540)
    for _ in range(100):
        response = b.receive()
        if response['type'] == 'frame' and response['encoded_height'] == 540 and response['encoded_width'] <= 960: break
    else: raise AssertionError('Viewport did not update')
    b.close()
    b = Bridge()
    last_sequence = 0
    b.call('start_stream', 0)
    restored=b.call('resume_window', resume_token=ticket)
    if a.switch_settings: assert restored['fps']==60 and restored['bitrate']==8000000
    b.call('window_group')
    while b.receive()['type'] != 'frame': pass
    b.call('close_window')
    b.call('ping', 0)
    print('PASS: unsolicited H264, ordered frames, control during video, viewport, resume and close', flush=True)
finally:
    b.close()
    output.close()

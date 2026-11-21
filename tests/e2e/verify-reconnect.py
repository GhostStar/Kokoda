#!/usr/bin/env python3
"""Exercise real capture teardown, idle runtime lifetime, and authenticated reconnect."""
import argparse
import json
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument('--client', required=True)
parser.add_argument('--address', required=True)
parser.add_argument('--pairing', required=True)
parser.add_argument('--window', required=True, help='Exact title of a visible test window')
parser.add_argument('--idle-seconds', type=int, default=180)
parser.add_argument('--cycles', type=int, default=20)
args = parser.parse_args()
if args.idle_seconds < 0 or args.cycles < 1:
    parser.error('idle-seconds must be nonnegative and cycles must be positive')


def exercise():
    process = subprocess.Popen(
        [args.client, args.address, args.pairing, '--bridge'],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
    )
    def exchange(request, allow_error=False):
        process.stdin.write(json.dumps(request) + '\n')
        process.stdin.flush()
        line = process.stdout.readline()
        if not line:
            raise RuntimeError(process.stderr.read())
        response = json.loads(line)
        if response.get('type') == 'error' and not allow_error:
            raise RuntimeError(response)
        return response
    try:
        missing = exchange({'command': 'select_window', 'id': 0}, allow_error=True)
        assert missing['type'] == 'error' and missing.get('selection_lost') is True
        assert exchange({'command': 'ping'})['type'] == 'pong', 'Window error closed connection'
        windows = exchange({'command': 'list_windows'})['windows']
        target = next(window for window in windows if window['title'] == args.window)
        for cycle in range(args.cycles):
            selected = exchange({'command': 'select_window', 'id': target['id']})
            assert selected['type'] == 'selected', selected
            for _ in range(50):
                frame = exchange({'command': 'frame', 'after': 0})
                if frame['type'] == 'frame':
                    break
                time.sleep(.1)
            assert frame['type'] == 'frame', 'No frame received'
            assert exchange({'command': 'close_window'})['type'] == 'closed'
            print(f'Capture/open/close {cycle + 1}/{args.cycles} passed', flush=True)
        assert exchange({'command': 'ping'})['type'] == 'pong'
    finally:
        process.stdin.close()
        try:
            process.wait(timeout=20)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
            raise


exercise()
print(f'Disconnected; waiting {args.idle_seconds}s for delayed runtime cleanup', flush=True)
remaining = args.idle_seconds
while remaining:
    interval = min(30, remaining)
    time.sleep(interval)
    remaining -= interval
    print(f'Idle wait: {remaining}s remaining', flush=True)
exercise()
print('PASS: capture teardown, idle interval, reconnect and capture restart', flush=True)

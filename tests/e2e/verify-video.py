"""Stream a selected synthetic test window; write packets for native decoder validation.
No inputs are sent. Close the GUI before running (single-client host).
"""
import argparse, base64, json, subprocess, time, select
from pathlib import Path
p=argparse.ArgumentParser()
p.add_argument('--client',required=True);p.add_argument('--address',required=True);p.add_argument('--pairing',required=True)
p.add_argument('--viewports',default='',help='Comma-separated backing-pixel viewports, applied every two seconds')
p.add_argument('--request-delay',type=float,default=0)
p.add_argument('--codec',choices=['images','h264'],default='h264');p.add_argument('--seconds',type=float,default=10)
p.add_argument('--fps',type=int,default=30);p.add_argument('--bitrate',type=int,default=2000000);p.add_argument('--max-width',type=int,default=1280)
p.add_argument('--output',required=True);p.add_argument('--window',default='Kokoda Video Test')
a=p.parse_args()
proc=subprocess.Popen([a.client,a.address,a.pairing,'--bridge'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
def exchange(value):
    proc.stdin.write(json.dumps(value).encode()+b'\n');proc.stdin.flush()
    if not select.select([proc.stdout],[],[],15)[0]: raise TimeoutError('host response')
    line=proc.stdout.readline()
    if not line:raise RuntimeError(proc.stderr.read().decode())
    value=json.loads(line)
    if value['type']=='error':raise RuntimeError(value)
    return value
try:
    windows=exchange({'command':'list_windows'})['windows']
    root=next(w for w in windows if w['title']==a.window)
    exchange({'command':'select_window','id':root['id'],'scope':'application','codec':a.codec,'fps':a.fps,'bitrate':a.bitrate,'max_width':a.max_width})
    members=exchange({'command':'window_group'})['windows'];root=next(w for w in members if w['id']==root['id'])
    viewports=[tuple(map(int,v.split('x'))) for v in a.viewports.split(',') if v]
    viewport_index=0;seen_sizes=set()
    start=time.monotonic();count=0;total=0;seq=0;latencies=[]
    with Path(a.output).open('w') as file:
        while time.monotonic()-start<a.seconds:
            if viewport_index < len(viewports) and time.monotonic()-start >= viewport_index*2:
                w,h=viewports[viewport_index]
                exchange({'command':'video_viewport','id':root['id'],'token':root['token'],'width':w,'height':h})
                viewport_index+=1
            time.sleep(a.request_delay)
            before=time.monotonic()
            r=exchange({'command':'frame','id':root['id'],'token':root['token'],'after':seq})
            if r['type']=='frame':
                assert r['sequence']>seq
                seen_sizes.add((r.get('encoded_width',r['width']),r.get('encoded_height',r['height'])))
                seq=r['sequence'];count+=1;total+=len(base64.b64decode(r['image']))
                latencies.append((time.monotonic()-before)*1000)
                file.write(json.dumps(r)+'\n')
            else:time.sleep(.002)
    elapsed=time.monotonic()-start
    assert count>=3,'Too few frames'
    print(json.dumps({'codec':a.codec,'encoded_sizes':sorted(seen_sizes),'frames':count,'seconds':round(elapsed,2),'fps':round(count/elapsed,2),'image_mbps':round(total*8/elapsed/1000000,3),'frame_request_ms':round(sum(latencies)/len(latencies),2),'source':[root['bounds']['right']-root['bounds']['left'],root['bounds']['bottom']-root['bounds']['top']]}))
    exchange({'command':'close_window'})
finally:
    proc.stdin.close()
    try:proc.wait(timeout=5)
    except subprocess.TimeoutExpired:proc.kill();proc.wait()

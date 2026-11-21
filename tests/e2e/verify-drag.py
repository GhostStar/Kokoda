#!/usr/bin/env python3
"""Exercise real drag/capture on synthetic Windows title bars and text only."""
import argparse,json,queue,subprocess,threading,time
from pathlib import Path
p=argparse.ArgumentParser()
for name in ('client','address','secret','state'):p.add_argument('--'+name,required=True)
a=p.parse_args()
bridge=subprocess.Popen([a.client,a.address,'--password-stdin','--stream-bridge'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
replies=queue.Queue(maxsize=32)
def reader():
    for line in bridge.stdout:replies.put(json.loads(line))
    replies.put({'type':'eof'})
threading.Thread(target=reader,daemon=True).start()
bridge.stdin.write(Path(a.secret).read_text()+'\n');bridge.stdin.flush()
def call(command,**fields):
    bridge.stdin.write(json.dumps(dict(command=command,**fields))+'\n');bridge.stdin.flush()
    while True:
        value=replies.get(timeout=15)
        assert value['type'] not in ('error','bridge_error','eof'),value.get('message',value['type'])
        if not value.get('event'):return value
def state():
    deadline=time.monotonic()+3
    while time.monotonic()<deadline:
        try:return json.loads(Path(a.state).read_text(encoding='utf-8-sig'))
        except (OSError,ValueError):time.sleep(.01)
    raise AssertionError('Synthetic state unavailable')
def settle():time.sleep(.10);return state()
def point(w,kind='caption'):
    return dict(x=(w[kind+'_x']-w['left'])/w['width'],y=(w[kind+'_y']-w['top'])/w['height'],width=w['width'],height=w['height'])
gesture=0
try:
    catalog=call('list_windows');assert catalog['anchored_pointer']
    root=next(w['id'] for w in catalog['windows'] if w['title']=='Kokoda Drag Root')
    call('start_stream')
    call('select_window',id=root,scope='application',codec='h264',fps=30,bitrate=1000000,max_width=640,encoder='software',alpha_video=True)
    members=call('window_group')['windows'];tokens={w['id']:w['token'] for w in members}
    def send(w,*events):return call('input',id=w['id'],token=tokens[w['id']],events=list(events))
    # Reproduce the former moving-origin error using the explicit legacy path.
    w=state()['root'];start=w['left'];pos=point(w)
    send(w,dict(kind='button',down=True,button=0,**pos));settle()
    for dx in (10,20,30,40):
        send(w,dict(kind='move',**dict(pos,x=pos['x']+dx/w['width'])));settle()
    old=state()['root']['left']-start
    send(w,dict(kind='release'));settle()
    print('Legacy moving-origin displacement for requested 40 px:',old,flush=True)
    assert old>60,'Legacy regression was not reproduced'
    errors=[]
    for name in ('root','custom'):
        w=state()[name];gesture+=1
        send(w,dict(kind='drag_begin',gesture=gesture,**point(w)))
        # Native caption tracking can postpone the fixture's UI timer until the
        # first movement. Derive the anchor from the exact requested pixel.
        settle();anchor=(w['caption_x'],w['caption_y'])
        for dx,dy in ((15,5),(30,10),(80,20),(80,20),(0,0),(-20,10),(40,0)):
            send(w,dict(kind='drag_move',gesture=gesture,dx=dx,dy=dy))
            current=settle();actual=current[name]
            error=max(abs(actual['left']-(w['left']+dx)),abs(actual['top']-(w['top']+dy)))
            errors.append(error);assert error<=2,('drag drift',name,error)
            assert abs(current['cursor_x']-anchor[0]-dx)<=1 and abs(current['cursor_y']-anchor[1]-dy)<=1
            geometry=next(v for v in call('window_group')['windows'] if v['id']==w['id'])['bounds']
            assert abs(geometry['left']-actual['left'])<=2
        send(w,dict(kind='drag_end',gesture=gesture,dx=40,dy=0));assert not settle()['left_down']
        # Native text capture must still permit dragging outside the viewer.
        w=state()[name];gesture+=1
        send(w,dict(kind='drag_begin',gesture=gesture,**point(w,'text')));settle()
        send(w,dict(kind='drag_move',gesture=gesture,dx=550,dy=0));selected=settle()
        send(w,dict(kind='drag_end',gesture=gesture,dx=550,dy=0));assert not settle()['left_down']
        assert selected[name]['selection']>0,'Outside-window native selection failed'
        # Explicit release cancels the anchor and the native pressed button.
        w=state()[name];gesture+=1
        send(w,dict(kind='drag_begin',gesture=gesture,**point(w)));settle()
        send(w,dict(kind='release'));assert not settle()['left_down']
    call('close_window')
    print('PASS: standard/custom titlebar drags, reversal, repeated sample, geometry reports, native outside selection and release; max error',max(errors),'px',flush=True)
finally:
    bridge.terminate()
    try:bridge.wait(timeout=5)
    except subprocess.TimeoutExpired:bridge.kill();bridge.wait()

#!/usr/bin/env python3
"""Only isolated window-roles fixture windows are manipulated."""
import argparse,json,queue,subprocess,threading,time
from pathlib import Path
p=argparse.ArgumentParser()
for name in ('client','address','secret','command','state'):p.add_argument('--'+name,required=True)
p.add_argument("--read-only",action="store_true")
a=p.parse_args()
bridge=subprocess.Popen([a.client,a.address,'--password-stdin','--stream-bridge'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
lines=queue.Queue(maxsize=128)
def reader():
    for line in bridge.stdout:lines.put(json.loads(line))
    lines.put({'type':'eof'})
threading.Thread(target=reader,daemon=True).start()
bridge.stdin.write(Path(a.secret).read_text()+'\n');bridge.stdin.flush()
def call(command,**fields):
    bridge.stdin.write(json.dumps(dict(command=command,session_id=1,**fields))+'\n');bridge.stdin.flush()
    while True:
        v=lines.get(timeout=20)
        assert v['type'] not in ('bridge_error','eof'),v.get('message',v['type'])
        if not v.get('event'):return v

def wait(message,fn):
    deadline=time.monotonic()+12
    while time.monotonic()<deadline:
        result=fn()
        if result:return result
        time.sleep(.10)
    raise AssertionError(message)
def group():
    value=call('window_group');assert value['type']=='window_group',value.get('message');return value

def by_title(title):return next((w for w in group()['windows'] if w['title']==title),None)
def action(w,name,**fields):return call('window_action',id=w['id'],token=w['token'],action=name,**fields)
def trigger(value):Path(a.command).write_text(str(time.monotonic_ns())+':'+value)
def state():
    for _ in range(30):
        try:return json.loads(Path(a.state).read_text(encoding='utf-8-sig'))
        except (OSError,ValueError):time.sleep(.03)
    raise AssertionError('Fixture state unavailable')
try:
    def discover():
        v=call('list_windows');assert v.get('window_management');return next((w for w in v['windows'] if w['title']=='Kokoda Roles Root'),None)
    root=wait('Root discovery',discover)
    assert call('select_window',id=root['id'],scope='application',window_management=True)['type']=='selected'
    if a.read_only:
        root=wait('Authorized root',lambda:by_title('Kokoda Roles Root'))
        for name in ('close','minimize','restore','maximize','activate','resize'):
            denied=action(root,name,width=720,height=420)
            assert denied.get('code')=='sharing_read_only',name
        trigger('child');wait('Child fixture started',lambda:state()['child_pid']>0);time.sleep(1)
        listed=call('list_windows');assert listed['read_only']
        assert all(w['title'] not in ('Kokoda Roles Independent','Kokoda Roles Child Process','Kokoda Roles Unrelated Process') for w in listed['windows'])
        assert by_title('Kokoda Roles Child Process') is None
        assert by_title('Kokoda Roles Root') is not None
        print('PASS: host rejects every window mutation in read-only mode; process following cannot expose denied independent/descendant windows',flush=True)
        raise SystemExit(0)
    members=wait('Initial windows ready',lambda:(lambda ws:ws if len([w for w in ws if w['title'].startswith('Kokoda Roles ')])>=5 else None)(group()['windows']));items={w['title']:w for w in members}
    root=items['Kokoda Roles Root'];independent=items['Kokoda Roles Independent'];tool=items['Kokoda Roles Tool'];fixed=items['Kokoda Roles Fixed'];owned=items['Kokoda Roles Owned App']
    assert root['presentation']['role']=='independent'
    assert independent['presentation']['role']=='independent'
    assert tool['presentation']['role']=='floating' and tool['presentation']['owner']==root['id']
    assert owned['presentation']['role']=='independent' and owned['presentation']['owner']==root['id']
    assert not fixed['presentation']['can_resize']
    assert action(root,'resize',width=0,height=300)['type']=='error'
    assert call('window_action',id=root['id'],token=root['token']+1000,action='close')['type']=='error'
    before=state()
    assert action(independent,'resize',width=720,height=420,revision=1,final=True)['type']=='window_action_ok'
    def sized():
        w=by_title('Kokoda Roles Independent');b=w['bounds'];return w if abs((b['right']-b['left'])-720)<=2 and abs((b['bottom']-b['top'])-420)<=2 else None
    wait('Remote physical resize',sized)
    assert action(tool,'resize',width=420,height=240,revision=2,final=True)['type']=='window_action_ok'
    wait('Owned resize',lambda:(lambda w: w if abs(w['bounds']['right']-w['bounds']['left']-420)<=2 else None)(by_title('Kokoda Roles Tool')))
    assert action(fixed,'resize',width=900,height=500)['type']=='error'
    after=state();assert (before['cursor_x'],before['cursor_y'])==(after['cursor_x'],after['cursor_y']),'Window resize moved the mouse'
    assert action(independent,'resize',width=64,height=64,revision=3,final=True)['type']=='window_action_ok'
    time.sleep(.3);w=by_title('Kokoda Roles Independent');assert w['bounds']['right']-w['bounds']['left']>=260,'Application minimum ignored'
    print('PASS: window roles, same-process independent taskbar window, bounded resize, native min-size, fixed-size refusal, stale token and stationary cursor',flush=True)
    assert action(independent,'minimize')['type']=='window_action_ok'
    wait('Remote minimize',lambda:by_title('Kokoda Roles Independent')['presentation']['minimized'])
    assert action(independent,'restore')['type']=='window_action_ok'
    wait('Remote restore',lambda:not by_title('Kokoda Roles Independent')['presentation']['minimized'])
    assert action(independent,'maximize')['type']=='window_action_ok'
    wait('Remote maximize',lambda:by_title('Kokoda Roles Independent')['presentation']['maximized'])
    assert action(independent,'restore')['type']=='window_action_ok'
    wait('Remote unmaximize',lambda:not by_title('Kokoda Roles Independent')['presentation']['maximized'])
    trigger('modal');modal=wait('Modal discovery',lambda:by_title('Kokoda Roles Modal'))
    assert modal['presentation']['role']=='modal' and not by_title('Kokoda Roles Root')['presentation']['enabled']
    assert action(root,'resize',width=600,height=400)['type']=='error'
    assert action(modal,'close')['type']=='window_action_ok'
    wait('Modal closed',lambda:by_title('Kokoda Roles Modal') is None)
    wait('Owner enabled',lambda:by_title('Kokoda Roles Root')['presentation']['enabled'])
    print('PASS: minimize/restore/maximize, modal owner rejection and native dialog close',flush=True)
    assert action(root,'activate')['type']=='window_action_ok'
    trigger('menu')
    menu=wait('Native menu discovered',lambda:next((w for w in group()['windows'] if w['presentation']['role']=='transient'),None))
    assert not menu['presentation']['can_resize'] and not menu['presentation']['can_minimize']
    response=call('input',id=menu['id'],token=menu['token'],events=[{'kind':'key','vk':27,'down':True},{'kind':'key','vk':27,'down':False}]);assert response['type']=='input_ok',response.get('message')
    wait('Native Escape dismissed menu',lambda:all(w['id']!=menu['id'] for w in group()['windows']))
    print('PASS: system menu classified as transient and receives input without foreground activation',flush=True)
    trigger('child');child=wait('New process window automatically discovered',lambda:by_title('Kokoda Roles Child Process'))
    assert child['process_id']!=root['process_id'] and child['presentation']['role']=='independent'
    assert by_title('Kokoda Roles Unrelated Process') is None
    assert any(w['title']=='Kokoda Roles Unrelated Process' for w in call('list_windows')['windows']),'Unrelated fixture missing from test'
    assert action(child,'resize',width=640,height=360,revision=4,final=True)['type']=='window_action_ok'
    assert action(child,'close')['type']=='window_action_ok'
    wait('Child process closed',lambda:by_title('Kokoda Roles Child Process') is None)
    assert action(root,'close')['type']=='window_action_ok'
    wait('Anchor promotion',lambda:(lambda g:g['root_id']!=root['id'] and any(w['id']==independent['id'] for w in g['windows']))(group()))
    assert action(independent,'close')['type']=='window_action_ok'
    print('PASS: verified descendant window joins automatically, unrelated same-executable process excluded, per-window close and surviving independent anchor',flush=True)
finally:
    trigger('stop')
    bridge.terminate()
    try:bridge.wait(timeout=5)
    except subprocess.TimeoutExpired:bridge.kill();bridge.wait()

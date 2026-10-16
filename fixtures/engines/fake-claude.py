#!/usr/bin/env python3
# TEST DATA. Deterministic native-boundary fixture; never calls an installed engine or a provider.
import json, os, subprocess, sys, time
if '--version' in sys.argv:
    print('2.1.301 (Claude Code)')
elif '--help' in sys.argv:
    print('--restricted --tools --mcp-config --strict-mcp-config --no-session-persistence --permission-prompts --include-partial-messages')
elif 'auth' in sys.argv:
    print(json.dumps({'loggedIn': True, 'accessToken': 'TEST DATA diagnostic discarded'}))
else:
    assert not any(k.startswith('VELLUM_') for k in os.environ), 'Vellum environment leaked'
    assert 'CLAUDECODE' not in os.environ
    assert '--restricted' in sys.argv and sys.argv[sys.argv.index('--tools')+1] == ''
    message=json.loads(sys.stdin.readline())
    text=message['message']['content'][-1]['text']
    if text == 'TEST DATA slow tree':
        child=subprocess.Popen([sys.executable, '-c', "import time;time.sleep(120)"])
        open('child.pid', 'w').write(str(child.pid))
        print(json.dumps({'type':'stream_event','event':{'type':'content_block_delta','delta':{'type':'text_delta','text':'Started'}}}), flush=True)
        time.sleep(120)
    elif text == 'TEST DATA truncated':
        print('{"type":', end='', flush=True)
    elif text == 'TEST DATA large':
        print('x' * (4*1024*1024+1), flush=True)
    elif text == 'TEST DATA stderr':
        sys.stderr.write('TEST DATA canary-secret\n' * 100000)
        print(json.dumps({'type':'result','subtype':'error_during_execution','is_error':True,'result':'TEST DATA canary-secret'}))
    else:
        print(json.dumps({'type':'result','subtype':'success','is_error':False,'result':'Done','usage':{'input_tokens':1,'output_tokens':1}}))

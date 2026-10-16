#!/usr/bin/env python3
# TEST DATA. A deterministic Codex subprocess. No installed engine or network.
import json, os, subprocess, sys, time
args=sys.argv[1:]
if "--version" in args:
    print("codex-cli 0.158.0"); sys.exit(0)
if args == ["login", "status"]:
    print("Logged in using ChatGPT", file=sys.stderr); sys.exit(0)
if "--help" in args:
    print("--listen" if "app-server" in args else "exec app-server --json --ephemeral --ignore-user-config --ignore-rules --sandbox"); sys.exit(0)
def emit(v):
    print(json.dumps(v), flush=True)
if "exec" in args:
    prompt=sys.stdin.read()
    emit({"type":"item.completed","item":{"id":"m","type":"agent_message","text":"Done"}})
    emit({"type":"turn.completed","usage":{"input_tokens":2,"output_tokens":1}})
    sys.exit(0)
for line in sys.stdin:
    v=json.loads(line); method=v.get("method")
    if method == "initialize": emit({"id":v["id"],"result":{}})
    elif method == "config/read": emit({"id":v["id"],"result":{"config":{}}})
    elif method == "thread/start":
        assert v["params"]["environments"]==[]
        assert v["params"]["ephemeral"] is True
        emit({"id":v["id"],"result":{"thread":{"id":"fixture-thread"}}})
    elif method == "turn/start":
        emit({"id":v["id"],"result":{"turn":{"id":"fixture-turn"}}})
        prompt=v["params"]["input"][0]["text"]
        if "slow tree" in prompt:
            child=subprocess.Popen([sys.executable,"-c","import time; time.sleep(120)"])
            open("child.pid","w").write(str(child.pid))
            emit({"method":"item/agentMessage/delta","params":{"threadId":"fixture-thread","turnId":"fixture-turn","itemId":"m","delta":"Working"}})
            time.sleep(120)
        elif "truncated" in prompt:
            sys.stdout.write('{"method":'); sys.stdout.flush(); sys.exit(0)
        elif "large" in prompt:
            print("x"*(4*1024*1024+1),flush=True); sys.exit(0)
        elif "stderr" in prompt:
            print("TEST DATA canary-secret",file=sys.stderr); sys.exit(1)
        else:
            emit({"method":"item/agentMessage/delta","params":{"threadId":"fixture-thread","turnId":"fixture-turn","itemId":"m","delta":"Done"}})
            emit({"method":"turn/completed","params":{"threadId":"fixture-thread","turn":{"id":"fixture-turn","status":"completed"}}})

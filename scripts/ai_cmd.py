#!/usr/bin/env python3
"""Simple CLI to send commands to ai_interface socket."""
import socket
import sys
import json

def main():
    if len(sys.argv) < 3:
        print("Usage: ai_cmd.py <session> <action> [target] [params_json]")
        print("Example: ai_cmd.py booking refresh_page customer '{\"page_dir\": \"/path\"}'")
        sys.exit(1)

    session = sys.argv[1]
    action = sys.argv[2]
    target = sys.argv[3] if len(sys.argv) > 3 else None
    params = json.loads(sys.argv[4]) if len(sys.argv) > 4 else {}

    socket_path = f"/tmp/sthalam/{session}/ai.sock"

    cmd = {"action": action, "params": params, "id": 1}
    if target:
        cmd["target"] = target

    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect(socket_path)
    sock.sendall((json.dumps(cmd) + "\n").encode())
    sock.settimeout(10)

    response = sock.recv(8192).decode()
    print(response)
    sock.close()

if __name__ == "__main__":
    main()

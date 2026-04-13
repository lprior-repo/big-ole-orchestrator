#!/usr/bin/env python3
"""
Signal delivery under load test: receives signal while doing IPC.
This script catches SIGTERM and writes a response before exiting.
Tests that signal delivery is handled correctly under IPC load.
"""
import os
import sys
import signal
import struct
import json

FD3 = 3
FD4 = 4

sigterm_received = False

def handle_sigterm(signum, frame):
    global sigterm_received
    sigterm_received = True

signal.signal(signal.SIGTERM, handle_sigterm)

try:
    length_bytes = os.read(FD3, 4)
    if len(length_bytes) < 4:
        sys.exit(0)
    msg_length = struct.unpack('>I', length_bytes)[0]
    os.read(FD3, min(16, msg_length))
    
    response = {
        "version": 1,
        "instance_id": "signal-test",
        "node_id": "signal-under-load",
        "result": {
            "success": {
                "output": json.dumps({"sigterm_received": sigterm_received})
            }
        }
    }
    response_bytes = json.dumps(response).encode('utf-8')
    os.write(FD4, struct.pack('>I', len(response_bytes)))
    os.write(FD4, response_bytes)
    sys.exit(0)
except Exception:
    sys.exit(1)
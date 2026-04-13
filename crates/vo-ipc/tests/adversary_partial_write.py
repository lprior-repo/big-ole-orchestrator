#!/usr/bin/env python3
"""
Partial write recovery test: simulates fd3 write partial success.
The parent tries to write a large payload but the child delays reading,
causing the write to partially succeed before the child processes it.
"""
import os
import sys
import struct
import json
import time

FD3 = 3
FD4 = 4

time.sleep(0.1)

try:
    length_bytes = os.read(FD3, 4)
    if len(length_bytes) < 4:
        sys.exit(0)
    msg_length = struct.unpack('>I', length_bytes)[0]
    
    total_read = 0
    while total_read < msg_length:
        chunk = os.read(FD3, min(1024, msg_length - total_read))
        if not chunk:
            break
        total_read += len(chunk)
    
    response = {
        "version": 1,
        "instance_id": "partial-write-test",
        "node_id": "partial-write-recovery",
        "result": {
            "success": {
                "output": json.dumps({"bytes_read": total_read, "expected": msg_length})
            }
        }
    }
    response_bytes = json.dumps(response).encode('utf-8')
    os.write(FD4, struct.pack('>I', len(response_bytes)))
    os.write(FD4, response_bytes)
    sys.exit(0)
except Exception:
    sys.exit(1)
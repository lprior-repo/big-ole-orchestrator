#!/usr/bin/env python3
"""
Process spawn edge case: fd3 closed before any read.
Tests error handling when fd3 is closed prematurely.
"""
import os
import sys
import struct
import json

os.close(3)

try:
    response = {
        "version": 1,
        "instance_id": "early-close",
        "node_id": "fd3-closed",
        "result": {
            "success": {
                "output": "fd3 was closed"
            }
        }
    }
    response_bytes = json.dumps(response).encode('utf-8')
    os.write(4, struct.pack('>I', len(response_bytes)))
    os.write(4, response_bytes)
except Exception:
    pass

sys.exit(0)
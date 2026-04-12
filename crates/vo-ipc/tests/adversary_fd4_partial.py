#!/usr/bin/env python3
"""
RED QUEEN adversarial test: IPC message ordering attack.

This script simulates a malicious subprocess that responds based on PARTIAL input
before fully receiving the request. This tests whether the parent process correctly
handles a response that was generated from incomplete request data.

Attack scenario:
1. Parent sends length-prefixed JSON request to subprocess
2. Subprocess reads only partial data (length header + some bytes)
3. Subprocess generates response based on partial data
4. Subprocess sends response and exits WITHOUT reading rest of request
5. Parent's subsequent write may get broken pipe, but response was already received

This tests the resilience of the IPC layer to ordering/partial-data attacks.
"""
import sys
import os
import struct
import json

FD3 = 3  # Parent -> Child (subprocess stdin, but remapped)
FD4 = 4  # Child -> Parent (subprocess stdout, but remapped)

# Read length prefix (4 bytes big-endian)
length_bytes = os.read(FD3, 4)
if len(length_bytes) < 4:
    sys.exit(0)

msg_length = struct.unpack('>I', length_bytes)[0]

# Read only PART of the payload (deliberately incomplete)
# Normal subprocess would read msg_length bytes, but we exit early
partial = os.read(FD3, min(1, msg_length))

# Construct a response based on PARTIAL input
# This simulates a subprocess that responds before fully receiving the request
response = {
    "version": 1,
    "instance_id": "adversary-response",
    "node_id": "partial-input-attack",
    "result": {
        "success": {
            "output": json.dumps({
                "attack": "partial_input_response",
                "bytes_read": len(length_bytes) + len(partial),
                "expected_total": 4 + msg_length,
                "partial_pct": (len(length_bytes) + len(partial)) / (4 + msg_length) * 100
            })
        }
    }
}

response_bytes = json.dumps(response).encode('utf-8')

# Write the response - this may race with parent's continued write
os.write(FD4, struct.pack('>I', len(response_bytes)))
os.write(FD4, response_bytes)

# Exit immediately without reading the rest of the input
# Parent's subsequent write to FD3 will get broken pipe
sys.exit(0)
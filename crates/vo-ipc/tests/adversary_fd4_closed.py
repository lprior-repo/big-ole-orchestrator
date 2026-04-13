#!/usr/bin/env python3
"""
Process spawn edge case: fd4 closed before any read.
Tests error handling when fd4 is closed prematurely.
"""
import os
import sys

os.close(4)

try:
    length_bytes = os.read(3, 4)
    if len(length_bytes) >= 4:
        msg_length = int.from_bytes(length_bytes, 'big')
        os.read(3, msg_length)
except Exception:
    pass

sys.exit(0)
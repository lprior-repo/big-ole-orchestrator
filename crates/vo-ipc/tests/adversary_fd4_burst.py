#!/usr/bin/env python3
"""
FD4 pipe burst test: writes massive data to fd4 to saturate the pipe buffer.
Tests that the parent correctly handles a child that produces excessive output.
"""
import os
import sys
import struct

FD4 = 4

try:
    chunk = b'y' * 65536
    length = 100 * len(chunk)
    os.write(FD4, struct.pack('>I', length))
    for _ in range(100):
        os.write(FD4, chunk)
    sys.exit(0)
except BrokenPipeError:
    sys.exit(42)
except Exception:
    sys.exit(1)
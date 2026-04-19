#!/usr/bin/env python3
"""
FD3 pipe burst test: writes massive data to fd3 to saturate the pipe buffer.
Tests that the parent correctly handles a child that writes excessively to fd3.
"""
import os
import sys

FD3 = 3

try:
    chunk = b'x' * 65536
    for _ in range(100):
        os.write(FD3, chunk)
    sys.exit(0)
except BrokenPipeError:
    sys.exit(42)
except Exception:
    sys.exit(1)
#!/usr/bin/env python3
import sys
import os
import struct

# FD4 is target for IPC output
fd4 = 4
try:
    # Write a huge length (1GB) but don't provide the bytes
    os.write(fd4, struct.pack('>I', 1024 * 1024 * 1024))
    # Exit immediately
    sys.exit(0)
except Exception:
    sys.exit(1)

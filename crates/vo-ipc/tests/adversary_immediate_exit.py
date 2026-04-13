#!/usr/bin/env python3
"""
Process spawn edge case: immediately exits without reading fd3 or writing fd4.
Tests that the IPC layer handles a subprocess that ignores its stdin/stdout.
"""
import sys
sys.exit(0)
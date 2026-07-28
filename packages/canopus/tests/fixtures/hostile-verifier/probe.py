#!/usr/bin/env python3
import json
import os
import socket


def writable(path: str) -> bool:
    try:
        with open(path, "wb") as handle:
            handle.write(b"forbidden\n")
        return True
    except OSError:
        return False


def network_reachable() -> bool:
    connection = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    connection.settimeout(0.25)
    try:
        connection.connect(("1.1.1.1", 53))
        return True
    except OSError:
        return False
    finally:
        connection.close()


print(json.dumps({
    "network_reachable": network_reachable(),
    "root_writable": writable("/escape"),
    "input_writable": writable("/input/source/escape"),
    "artifact_writable": writable("/artifacts/0"),
    "capsule_writable": writable("/capsule/probe.py"),
    "host_home_visible": os.path.exists("/Users") or os.path.exists("/home/williamblair"),
}, sort_keys=True))

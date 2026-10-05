#!/usr/bin/env python3

import base64
import hashlib
import sys

from cryptography.fernet import Fernet

def key_from_passphrase(passphrase: str) -> bytes:
    digest = hashlib.sha256(passphrase.encode()).digest()
    return base64.urlsafe_b64encode(digest)

if __name__ == "__main__":
    if len(sys.argv) != 3 or sys.argv[1] not in ("encode", "decode"):
        print(__doc__, file=sys.stderr)
        sys.exit(1)

    op = sys.argv[1]
    key = key_from_passphrase(sys.argv[2])
    f = Fernet(key)
    data = sys.stdin.read().strip()

    if op == "encode":
        print(f.encrypt(data.encode()).decode())
    else:
        print(f.decrypt(data.encode()).decode())

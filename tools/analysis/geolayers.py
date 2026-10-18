"""For each cell: does set B have any layer within 16 of set A's top layer (and vice versa)?"""
import os
import struct
import sys


def cells(path):
    """Yield list of layer heights per cell in file order (block-major, x*8+y inside block)."""
    buf = open(path, 'rb').read()
    pos = 0
    out = []
    for _ in range(256 * 256):
        t = buf[pos]
        pos += 1
        if t == 0:
            h = struct.unpack_from('<h', buf, pos)[0]
            pos += 2
            out.extend([[h]] * 64)
        elif t == 1:
            vals = struct.unpack_from('<64h', buf, pos)
            pos += 128
            out.extend([[(v & -16) >> 1] for v in vals])
        else:
            for _ in range(64):
                n = buf[pos]
                vals = struct.unpack_from(f'<{n}h', buf, pos + 1)
                pos += 1 + 2 * n
                out.append([(v & -16) >> 1 for v in vals])
    return out


def contained(src, dst):
    hit = 0
    for a, b in zip(src, dst):
        top = max(a)
        if any(abs(top - z) <= 16 for z in b):
            hit += 1
    return 100 * hit / len(src)


for name in sys.argv[3:]:
    a = cells(os.path.join(sys.argv[1], name + '.l2j'))
    b = cells(os.path.join(sys.argv[2], name + '.l2j'))
    print(f'{name}: A-top found in B {contained(a, b):.1f}%  B-top found in A {contained(b, a):.1f}%', flush=True)

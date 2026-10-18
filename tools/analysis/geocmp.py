"""Compare two L2J geodata (.l2j) directories region by region.

Format (from L2JMobius geoengine): 256x256 blocks per region, each block is
type byte + payload. Flat: i16 height. Complex: 64 x i16 cell data
(nswe = low 4 bits, height = (data & 0xFFF0) >> 1). Multilayer: per cell
u8 layer count + count x i16 layer data. Cell index inside block = x*8+y.
"""
import multiprocessing
import os
import struct
import sys
import numpy as np

BLOCKS = 256
CELLS = 8


def decode(path):
    buf = open(path, 'rb').read()
    layers = np.zeros((BLOCKS * CELLS, BLOCKS * CELLS), np.uint8)
    top_z = np.zeros_like(layers, dtype=np.int16)
    top_nswe = np.zeros_like(layers, dtype=np.uint8)
    types = [0, 0, 0]
    pos = 0
    for bx in range(BLOCKS):
        for by in range(BLOCKS):
            t = buf[pos]
            pos += 1
            types[t] += 1
            x0, y0 = bx * CELLS, by * CELLS
            if t == 0:
                h = int.from_bytes(buf[pos:pos + 2], 'little', signed=True)
                pos += 2
                layers[x0:x0 + 8, y0:y0 + 8] = 1
                top_z[x0:x0 + 8, y0:y0 + 8] = h
                top_nswe[x0:x0 + 8, y0:y0 + 8] = 15
            elif t == 1:
                d = np.frombuffer(buf, '<i2', 64, pos).reshape(8, 8)
                pos += 128
                layers[x0:x0 + 8, y0:y0 + 8] = 1
                top_z[x0:x0 + 8, y0:y0 + 8] = (d & np.int16(-16)) >> 1
                top_nswe[x0:x0 + 8, y0:y0 + 8] = d & 0x0F
            elif t == 2:
                ns, zs, ws = [], [], []
                for _ in range(64):
                    n = buf[pos]
                    vals = struct.unpack_from(f'<{n}h', buf, pos + 1)
                    pos += 1 + 2 * n
                    best = max(vals, key=lambda v: (v & -16) >> 1)
                    ns.append(n)
                    zs.append((best & -16) >> 1)
                    ws.append(best & 0x0F)
                layers[x0:x0 + 8, y0:y0 + 8] = np.array(ns, np.uint8).reshape(8, 8)
                top_z[x0:x0 + 8, y0:y0 + 8] = np.array(zs, np.int16).reshape(8, 8)
                top_nswe[x0:x0 + 8, y0:y0 + 8] = np.array(ws, np.uint8).reshape(8, 8)
            else:
                raise ValueError(f'{path}: bad block type {t} at {pos - 1}')
    assert pos == len(buf), f'{path}: {len(buf) - pos} trailing bytes'
    return types, layers, top_z, top_nswe


def compare(job):
    dir_a, dir_b, name = job
    ta, la, za, na = decode(os.path.join(dir_a, name))
    tb, lb, zb, nb = decode(os.path.join(dir_b, name))
    dz = np.abs(za.astype(np.int32) - zb.astype(np.int32))
    total = la.size
    return (f'{name[:-4]},{ta[0]},{ta[1]},{ta[2]},{tb[0]},{tb[1]},{tb[2]},'
            f'{la.mean():.3f},{lb.mean():.3f},'
            f'{100 * (la != lb).sum() / total:.2f},{dz.mean():.1f},'
            f'{100 * (dz > 16).sum() / total:.2f},{100 * (na != nb).sum() / total:.2f}')


if __name__ == '__main__':
    a, b = sys.argv[1], sys.argv[2]
    names = sys.argv[3:] or sorted(f for f in os.listdir(a) if f.endswith('.l2j'))
    print('region,a_flat,a_cplx,a_multi,b_flat,b_cplx,b_multi,a_avg_layers,b_avg_layers,'
          'pct_layercount_diff,mean_abs_dz,pct_dz_gt16,pct_nswe_diff')
    with multiprocessing.Pool(max(1, os.cpu_count() - 2)) as pool:
        for line in pool.imap(compare, [(a, b, n) for n in names]):
            print(line, flush=True)

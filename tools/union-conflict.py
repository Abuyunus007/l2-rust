"""Resolve git conflict hunks by keeping both sides (ours, then theirs), dropping exact duplicates.

Only for files where parallel cards append independent lines (lib.rs / mod.rs `mod` and `use` lines).
"""
import sys

path = sys.argv[1]
out, ours, theirs, state = [], [], [], None
for line in open(path, encoding='utf-8').read().split('\n'):
    if line.startswith('<<<<<<< '):
        state, ours, theirs = 'ours', [], []
    elif line.startswith('=======') and state == 'ours':
        state = 'theirs'
    elif line.startswith('>>>>>>> ') and state == 'theirs':
        out.extend(ours + [l for l in theirs if l not in ours])
        state = None
    elif state == 'ours':
        ours.append(line)
    elif state == 'theirs':
        theirs.append(line)
    else:
        out.append(line)
open(path, 'w', encoding='utf-8', newline='\n').write('\n'.join(out))

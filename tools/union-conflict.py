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
        # Drop only repeated declaration lines (`mod x;`, `use ...;`, `pub use ...;`, a Cargo.toml
        # key): a repeated `}` or `];` closes code that both sides need.
        def is_decl(l):
            t = l.strip()
            return (t.endswith(';') and (t.startswith(('mod ', 'pub mod ', 'use ', 'pub use ', 'pub(crate) mod ', 'pub(crate) use ')))) or ('=' in t and not t.startswith(('//', '#', '}', ')', ']')) and path.endswith('.toml'))
        out.extend(ours + [l for l in theirs if not (l in ours and is_decl(l))])
        state = None
    elif state == 'ours':
        ours.append(line)
    elif state == 'theirs':
        theirs.append(line)
    else:
        out.append(line)
open(path, 'w', encoding='utf-8', newline='\n').write('\n'.join(out))

# Cargo.toml: a union can list the same dependency twice when both sides added it next to
# different neighbours. Keep the first line of each key within a [section].
if path.endswith('Cargo.toml'):
    import re
    text = open(path, encoding='utf-8').read().split('\n')
    seen, section, kept = set(), None, []
    for line in text:
        if line.startswith('['):
            section, seen = line, set()
        m = re.match(r'^([A-Za-z0-9_-]+)(\.workspace)?\s*=', line)
        if m and section is not None:
            if m.group(1) in seen:
                continue
            seen.add(m.group(1))
        kept.append(line)
    open(path, 'w', encoding='utf-8', newline='\n').write('\n'.join(kept))

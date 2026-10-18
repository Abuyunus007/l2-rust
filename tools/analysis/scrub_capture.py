"""Scrubs an l2-proxy JSONL capture so it can be committed as a test fixture (M2-CAP-01).

Replaces every given secret string (account names, character names, clan names, ...) and every
given IPv4 address with a placeholder of the SAME length, in the decoded `fields` and in the raw
payload hex (as UTF-16LE, ASCII and raw IP bytes). Equal length keeps every packet layout and
length intact, so byte-level tests still work.

Usage:
  python -I scrub_capture.py IN.jsonl OUT.jsonl --secret yunus --secret Hero --ip 192.168.178.145
  python -I scrub_capture.py IN.jsonl OUT.jsonl --secrets-file secrets.txt   (one per line; "ip:a.b.c.d" for IPs)

After scrubbing it reports any remaining occurrence (fails with exit code 1 if one is found).
"""
import argparse
import ipaddress
import json
import sys


def placeholder(secret: str) -> str:
    # Same number of UTF-16 code units, letters only, deterministic per length.
    base = "Xscrubbedname"
    out = (base * (len(secret) // len(base) + 1))[: len(secret)]
    return out[0].upper() + out[1:] if out else out


def replacements(secrets, ips):
    """(old_bytes, new_bytes) pairs to apply to raw payloads."""
    pairs = []
    for s in secrets:
        p = placeholder(s)
        for enc in ("utf-16-le", "utf-8"):
            pairs.append((s.encode(enc), p.encode(enc)))
            pairs.append((s.lower().encode(enc), p.lower().encode(enc)))
            pairs.append((s.capitalize().encode(enc), p.capitalize().encode(enc)))
    for i, ip in enumerate(ips):
        fake = ipaddress.IPv4Address(f"10.66.{i // 250}.{i % 250 + 1}")
        pairs.append((ipaddress.IPv4Address(ip).packed, fake.packed))
        pairs.append((ip.encode(), str(fake).encode().ljust(len(ip.encode()), b" ")[: len(ip.encode())]))
    # Longer secrets first so a name containing another name is replaced whole.
    return sorted(set(pairs), key=lambda p: -len(p[0]))


def scrub_value(value, secrets, ips):
    if isinstance(value, str):
        for s in sorted(secrets, key=len, reverse=True):
            for variant in {s, s.lower(), s.capitalize()}:
                value = value.replace(variant, placeholder(variant))
        for i, ip in enumerate(ips):
            value = value.replace(ip, f"10.66.{i // 250}.{i % 250 + 1}")
        return value
    if isinstance(value, list):
        return [scrub_value(v, secrets, ips) for v in value]
    if isinstance(value, dict):
        return {k: scrub_value(v, secrets, ips) for k, v in value.items()}
    return value


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("input")
    ap.add_argument("output")
    ap.add_argument("--secret", action="append", default=[])
    ap.add_argument("--ip", action="append", default=[])
    ap.add_argument("--secrets-file")
    args = ap.parse_args()
    secrets, ips = list(args.secret), list(args.ip)
    if args.secrets_file:
        for line in open(args.secrets_file, encoding="utf-8"):
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            (ips if line.startswith("ip:") else secrets).append(line[3:] if line.startswith("ip:") else line)
    secrets = [s for s in secrets if len(s) >= 3]  # very short strings would hit random bytes
    pairs = replacements(secrets, ips)

    out_lines = []
    for line in open(args.input, encoding="utf-8"):
        rec = json.loads(line)
        if "raw" in rec and rec["raw"]:
            raw = bytes.fromhex(rec["raw"])
            for old, new in pairs:
                raw = raw.replace(old, new)
            rec["raw"] = raw.hex()
        for key in ("fields", "name"):
            if key in rec:
                rec[key] = scrub_value(rec[key], secrets, ips)
        out_lines.append(json.dumps(rec, separators=(",", ":"), ensure_ascii=False))
    text = "\n".join(out_lines) + "\n"

    leftovers = [old for old, _ in pairs if old and old in bytes.fromhex("".join(json.loads(l).get("raw", "") for l in out_lines))]
    leftovers += [s for s in secrets if s.lower() in text.lower()]
    open(args.output, "w", encoding="utf-8", newline="\n").write(text)
    if leftovers:
        print("WARNING: still found after scrubbing:", len(leftovers), "occurrence kinds", file=sys.stderr)
        sys.exit(1)
    print(f"scrubbed {len(out_lines)} records -> {args.output}")


if __name__ == "__main__":
    main()

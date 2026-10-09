#!/usr/bin/env python3
"""Author exactly two small QuickTime containers from generated AVC samples.

No AVAssetWriter movie or existing MOV is used as a template. Elementary
samples/configuration come from GenerateSamples.swift. This is a fixed-size
diagnostic author, not an input parser or product media writer.
"""
import hashlib
import json
from pathlib import Path
import struct
import sys


def u32(*values):
    return struct.pack(">" + "I" * len(values), *values)


def u16(*values):
    return struct.pack(">" + "H" * len(values), *values)


def atom(kind, payload):
    assert len(kind) == 4 and len(payload) < 1_000_000
    return u32(len(payload) + 8) + kind.encode("ascii") + payload


def full(kind, payload=b"", flags=0):
    return atom(kind, u32(flags) + payload)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def handler(component, subtype, name):
    label = name.encode("ascii")
    return full("hdlr", component + subtype + b"appl" + bytes(8) + bytes([len(label)]) + label)


def main():
    samples_dir, output = map(Path, sys.argv[1:])
    output.mkdir(mode=0o700, exist_ok=False)
    samples = [(samples_dir / f"frame-{i}.avc").read_bytes() for i in range(3)]
    avcc = (samples_dir / "avcC.bin").read_bytes()
    assert all(0 < len(s) < 100_000 for s in samples)
    assert 7 < len(avcc) < 4096 and avcc[0] == 1
    matrix = u32(0x10000, 0, 0, 0, 0x10000, 0, 0, 0, 0x40000000)
    ftyp = atom("ftyp", b"qt  " + u32(0) + b"qt  ")
    media = b"".join(samples)
    mdat = atom("mdat", media)
    first_offset = len(ftyp) + 8
    compressor = b"H.264"
    description = (bytes(6) + u16(1, 0, 0) + b"appl" + u32(0, 0x200)
                   + u16(64, 48) + u32(72 << 16, 72 << 16, 0) + u16(1)
                   + bytes([len(compressor)]) + compressor + bytes(31 - len(compressor))
                   + u16(24, 0xFFFF) + atom("avcC", avcc))
    stsd = full("stsd", u32(1) + atom("avc1", description))
    stts = full("stts", u32(1, 3, 60))
    stsc = full("stsc", u32(1, 1, 3, 1))
    stsz = full("stsz", u32(0, 3, *(len(s) for s in samples)))
    stco = full("stco", u32(1, first_offset))
    stss = full("stss", u32(3, 1, 2, 3))
    dinf = atom("dinf", full("dref", u32(1) + full("alis", flags=1)))
    cases = []
    for name, offset in [("zero-origin", 0), ("positive-composition", 60)]:
        # Decode duration is 180 ticks; presentation end is 180 + offset.
        # All declared movie/track/media extents include the last image end.
        end = 180 + offset
        mvhd = full("mvhd", u32(0, 0, 600, end, 0x10000) + u16(0x100)
                    + bytes(10) + matrix + bytes(24) + u32(2))
        tkhd = full("tkhd", u32(0, 0, 1, 0, end) + bytes(8) + u16(0, 0, 0, 0)
                    + matrix + u32(64 << 16, 48 << 16), flags=7)
        mdhd = full("mdhd", u32(0, 0, 600, end) + u16(0, 0))
        ctts = full("ctts", u32(1, 3, offset))
        stbl = atom("stbl", stsd + stts + ctts + stsc + stsz + stco + stss)
        minf = atom("minf", full("vmhd", u16(0, 0, 0, 0), flags=1)
                    + handler(b"dhlr", b"alis", "Data") + dinf + stbl)
        mdia = atom("mdia", mdhd + handler(b"mhlr", b"vide", "Diagnostic Video") + minf)
        # No edts/elst or cslg atom is present in either independently authored movie.
        data = ftyp + mdat + atom("moov", mvhd + atom("trak", tkhd + mdia))
        path = output / (name + ".mov")
        with path.open("xb") as f:
            f.write(data)
        path.chmod(0o444)
        cases.append({"name": name, "path": path.name, "sha256": sha(data),
            "bytes": len(data), "timescale": 600, "decode_duration_ticks": 180,
            "presentation_offset_ticks": offset, "presentation_end_ticks": end,
            "composition_ticks": [offset + 60 * i for i in range(3)],
            "mdat_sha256": sha(media), "sample_sha256": [sha(s) for s in samples],
            "sample_sizes": [len(s) for s in samples], "chunk_offset": first_offset,
            "avcC_sha256": sha(avcc), "edits": [], "cslg_present": False})
    print(json.dumps({"cases": cases}, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Read-only independent verifier for the exact two bounded generated MOVs."""
from fractions import Fraction
import hashlib
import json
from pathlib import Path
import struct
import sys


def sha(data):
    return hashlib.sha256(data).hexdigest()


def uint(data, pos, size=4):
    return int.from_bytes(data[pos:pos + size], "big")


def atoms(data, start, end):
    result = {}
    while start < end:
        assert start + 8 <= end, "short atom header"
        size = uint(data, start)
        kind = data[start + 4:start + 8].decode("ascii")
        assert 8 <= size <= end - start, "invalid/extended atom size"
        assert kind not in result, "duplicate atom"
        result[kind] = (start, start + 8, start + size)
        start += size
    assert start == end
    return result


def children(data, item):
    return atoms(data, item[1], item[2])


def table(data, item, columns):
    _, p, end = item
    assert uint(data, p) == 0, "expected v0/zero-flags table"
    count = uint(data, p + 4)
    assert 0 < count <= 8 and p + 8 + 4 * columns * count == end
    return [tuple(uint(data, p + 8 + 4 * (i * columns + j)) for j in range(columns))
            for i in range(count)]


def expanded(entries):
    assert sum(count for count, _ in entries) == 3
    return [value for count, value in entries for _ in range(count)]


def parse(path, samples_dir, offset):
    data = path.read_bytes()
    assert 0 < len(data) < 500_000
    top = atoms(data, 0, len(data))
    assert list(top) == ["ftyp", "mdat", "moov"]
    assert data[top["ftyp"][1]:top["ftyp"][2]] == b"qt  " + bytes(4) + b"qt  "
    moov = children(data, top["moov"])
    assert set(moov) == {"mvhd", "trak"}
    trak = children(data, moov["trak"])
    assert set(trak) == {"tkhd", "mdia"}, "edit lists are forbidden in this pair"
    mdia = children(data, trak["mdia"])
    assert set(mdia) == {"mdhd", "hdlr", "minf"}
    minf = children(data, mdia["minf"])
    assert set(minf) == {"vmhd", "hdlr", "dinf", "stbl"}
    stbl = children(data, minf["stbl"])
    assert set(stbl) == {"stsd", "stts", "ctts", "stsc", "stsz", "stco", "stss"}
    scales = [uint(data, a[1] + 12) for a in (moov["mvhd"], mdia["mdhd"])]
    ends = [uint(data, a[1] + 16) for a in (moov["mvhd"], mdia["mdhd"])]
    ends.append(uint(data, trak["tkhd"][1] + 20))
    assert scales == [600, 600] and ends == [180 + offset] * 3
    assert uint(data, trak["tkhd"][1] + 12) == 1
    assert data[mdia["hdlr"][1] + 8:mdia["hdlr"][1] + 12] == b"vide"
    dinf = children(data, minf["dinf"])
    assert set(dinf) == {"dref"}
    _, p, end = dinf["dref"]
    assert uint(data, p) == 0 and uint(data, p + 4) == 1
    refs = atoms(data, p + 8, end)
    assert set(refs) == {"alis"}
    a = refs["alis"]
    assert a[2] - a[1] == 4 and uint(data, a[1]) == 1, "external reference"
    _, p, end = stbl["stsd"]
    assert uint(data, p) == 0 and uint(data, p + 4) == 1
    entries = atoms(data, p + 8, end)
    assert set(entries) == {"avc1"}
    _, p, end = entries["avc1"]
    assert uint(data, p + 6, 2) == 1
    assert [uint(data, p + 24, 2), uint(data, p + 26, 2)] == [64, 48]
    extensions = atoms(data, p + 78, end)
    assert set(extensions) == {"avcC"}
    avc = extensions["avcC"]
    avcc = data[avc[1]:avc[2]]
    assert avcc == (samples_dir / "avcC.bin").read_bytes()
    assert avcc[0] == 1 and avcc[1] == 66, "expected baseline AVC configuration"
    nal_length_bytes = (avcc[4] & 3) + 1
    assert nal_length_bytes in (1, 2, 4)
    p = 6
    parameter_types = []
    for _ in range(avcc[5] & 31):
        size = uint(avcc, p, 2); p += 2
        assert size and p + size <= len(avcc)
        parameter_types.append(avcc[p] & 31); p += size
    pps_count = avcc[p]; p += 1
    for _ in range(pps_count):
        size = uint(avcc, p, 2); p += 2
        assert size and p + size <= len(avcc)
        parameter_types.append(avcc[p] & 31); p += size
    assert p == len(avcc) and parameter_types == [7, 8]
    deltas = expanded(table(data, stbl["stts"], 2))
    offsets = expanded(table(data, stbl["ctts"], 2))
    assert deltas == [60] * 3 and offsets == [offset] * 3
    assert table(data, stbl["stsc"], 3) == [(1, 3, 1)]
    assert table(data, stbl["stss"], 1) == [(1,), (2,), (3,)]
    chunk = table(data, stbl["stco"], 1)
    assert chunk == [(top["mdat"][1],)]
    _, p, end = stbl["stsz"]
    assert uint(data, p) == 0 and uint(data, p + 4) == 0 and uint(data, p + 8) == 3
    assert p + 24 == end
    sizes = [uint(data, p + 12 + 4 * i) for i in range(3)]
    current = top["mdat"][1]
    sample_hashes, nal_types = [], []
    for i, size in enumerate(sizes):
        assert size > 0 and current + size <= top["mdat"][2]
        sample = data[current:current + size]
        assert sample == (samples_dir / f"frame-{i}.avc").read_bytes()
        sample_hashes.append(sha(sample)); current += size
        q, types = 0, []
        while q < len(sample):
            assert q + nal_length_bytes <= len(sample)
            length = uint(sample, q, nal_length_bytes); q += nal_length_bytes
            assert 0 < length <= len(sample) - q
            types.append(sample[q] & 31); q += length
        assert q == len(sample) and 5 in types and 1 not in types, "expected all-intra IDR samples"
        nal_types.append(types)
    assert current == top["mdat"][2], "unreferenced/truncated media bytes"
    decode = [sum(deltas[:i]) for i in range(3)]
    composition = [d + c for d, c in zip(decode, offsets)]
    return {"file": path.name, "sha256": sha(data), "bytes": len(data),
        "timescale": 600, "movie_track_media_end_ticks": ends,
        "decode_ticks": decode, "composition_ticks": composition,
        "decode_duration_ticks": sum(deltas), "sample_duration_ticks": deltas,
        "composition_offsets": offsets, "edits": [], "cslg_present": False,
        "reference": "one self-contained alis entry, flags=1, no external data",
        "sample_sha256": sample_hashes, "sample_sizes": sizes,
        "mdat_sha256": sha(data[top["mdat"][1]:top["mdat"][2]]),
        "avcC_sha256": sha(avcc), "parameter_nal_types": parameter_types,
        "sample_nal_types": nal_types, "chunk_offset": chunk[0][0],
        "composition_seconds_exact": [str(Fraction(t, 600)) for t in composition]}


def verify(directory):
    expected = json.loads((directory / "author-result.json").read_text())["cases"]
    result = [parse(directory / "movies" / (name + ".mov"), directory / "samples", offset)
              for name, offset in [("zero-origin", 0), ("positive-composition", 60)]]
    for actual, intended in zip(result, expected):
        for key in ["sha256", "bytes", "sample_sha256", "sample_sizes", "mdat_sha256", "avcC_sha256", "chunk_offset"]:
            assert actual[key] == intended[key], (key, actual[key], intended[key])
        assert actual["composition_ticks"] == intended["composition_ticks"]
    assert result[0]["sample_sha256"] == result[1]["sample_sha256"]
    assert result[0]["mdat_sha256"] == result[1]["mdat_sha256"]
    return {"cases": result, "identical_encoded_samples": True, "native_precondition_not_yet_assessed": True}


if __name__ == "__main__":
    print(json.dumps(verify(Path(sys.argv[1])), indent=2, sort_keys=True))

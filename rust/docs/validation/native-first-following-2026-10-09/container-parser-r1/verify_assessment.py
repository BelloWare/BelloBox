"""Read-only verification of this fixed, generated MOV assessment (not a MOV validator)."""
from pathlib import Path
import hashlib
import json
import struct
import sys


def digest(data):
    return hashlib.sha256(data).hexdigest()


def timeline(data):
    assert len(data) < 1024 * 1024
    records = {}

    def walk(start, end, parents=()):
        assert len(parents) <= 8
        while start < end:
            assert start + 8 <= end
            size, kind = struct.unpack_from(">I4s", data, start)
            header = 8
            if size == 1:
                size = struct.unpack_from(">Q", data, start + 8)[0]
                header = 16
            elif size == 0:
                size = end - start
            assert header <= size <= end - start
            path = parents + (kind.decode("ascii"),)
            payload = data[start + header:start + size]
            if kind in (b"moov", b"trak", b"mdia", b"minf", b"stbl", b"edts"):
                walk(start + header, start + size, path)
            if kind in (b"mvhd", b"mdhd"):
                version = payload[0]
                offset = 20 if version == 1 else 12
                scale = struct.unpack_from(">I", payload, offset)[0]
                duration = struct.unpack_from(">Q" if version == 1 else ">I", payload, offset + 4)[0]
                records["/".join(path)] = {"timescale": scale, "duration": duration}
            elif kind in (b"stts", b"ctts"):
                version, count = payload[0], struct.unpack_from(">I", payload, 4)[0]
                assert count <= 16 and len(payload) == 8 + count * 8
                fmt = ">Ii" if kind == b"ctts" and version == 1 else ">II"
                records["/".join(path)] = {"version": version, "entries": [
                    list(struct.unpack_from(fmt, payload, 8 + 8 * i)) for i in range(count)]}
            elif kind == b"elst":
                version, count = payload[0], struct.unpack_from(">I", payload, 4)[0]
                assert version in (0, 1) and count <= 8
                fmt = ">Qqhh" if version == 1 else ">Iihh"
                width = struct.calcsize(fmt)
                assert len(payload) == 8 + count * width
                records["/".join(path)] = {"version": version, "entries": [
                    dict(zip(("movie_duration", "media_time", "rate_integer", "rate_fraction"),
                             struct.unpack_from(fmt, payload, 8 + width * i)))
                    for i in range(count)]}
            elif kind == b"stsz":
                sample_size, count = struct.unpack_from(">II", payload, 4)
                assert count <= 8
                records["/".join(path)] = {"sample_size": sample_size, "sample_count": count}
            start += size
        assert start == end

    walk(0, len(data))
    # All generated cases have one video track. stts/ctts use media ticks;
    # elst durations use movie ticks and media_time uses media ticks.
    prefix = "moov/trak/mdia/minf/stbl/"
    stts = records[prefix + "stts"]["entries"]
    ctts = records.get(prefix + "ctts", {"entries": []})["entries"]
    deltas = [delta for count, delta in stts for _ in range(count)]
    offsets = [offset for count, offset in ctts for _ in range(count)] or [0] * len(deltas)
    assert len(deltas) == len(offsets) == records[prefix + "stsz"]["sample_count"]
    decode, samples = 0, []
    for delta, offset in zip(deltas, offsets):
        samples.append({"dts_media_ticks": decode, "pts_media_ticks": decode + offset,
                        "duration_media_ticks": delta})
        decode += delta
    return {"atoms": records, "physical_sample_timing": samples}


root = Path(sys.argv[1])
trace = json.loads((root / "swift-timing.json").read_text())
assert "error" not in trace
expected = {
    "regular": [0, 0.1, 0.2, 0.3],
    "delayed-first-control": [0, 0.1, 0.2, 0.3],
    "session-at-first": [0, 0.1, 0.2],
    "negative-session": [0, 0.1, 0.2, 0.3],
    "long-leading-gap": [0, 3.1, 3.2, 3.3],
    "explicit-empty-edit": [0, 0.1, 0.2, 0.3, 0.4],
}
assert [row["case"] for row in trace["cases"]] == list(expected)
black = digest(bytes([0, 0, 0, 255]) * (64 * 48))
results = []
for row in trace["cases"]:
    name = row["case"]
    path = root / "generated-movies" / (name + ".mov")
    writer = root / "generated-movies" / (name + ".writer.mov")
    data = path.read_bytes()
    assert digest(data) == row["file_sha256"] and len(data) == row["file_bytes"]
    assert digest(writer.read_bytes()) == row["writer_sha256"]
    assert row["source_unchanged_after_read"]
    for reader in (row["compressed"], row["decoded"]):
        assert reader["reader_status"] == 2  # AVAssetReaderStatusCompleted
    frames = row["decoded"]["samples"]
    pts = [sample["pts"]["seconds"] for sample in frames]
    assert pts == expected[name]
    assert all(sample["has_image"] and sample["sample_count"] == 1 for sample in frames)
    assert all((sample["width"], sample["height"]) == (64, 48) for sample in frames)
    positive_gap_precondition = bool(pts) and pts[0] > 0
    assert not positive_gap_precondition  # Never count normalized PTS0 as proof.
    has_empty = any(segment["empty"] for segment in row["decoded"]["segments"])
    if has_empty:
        assert frames[0]["rgba_sha256"] == black
    raw = timeline(data)
    sample_count = len(raw["physical_sample_timing"])
    assert sample_count == len(row["submitted_pts"])
    assert sum(sample["sample_count"] for sample in row["compressed"]["samples"]) == sample_count
    results.append({"case": name, "movie_sha256": row["file_sha256"],
                    "decoded_pts": pts, "encoded_sample_count": sample_count,
                    "leading_empty_container_edit": has_empty,
                    "first_decoded_frame_is_opaque_black": frames[0]["rgba_sha256"] == black,
                    "positive_gap_precondition": positive_gap_precondition,
                    "raw_container": raw})
print(json.dumps({"result": "verified limitation for these six generated cases",
                  "native_first_following_coverage": False,
                  "opaque_black_rgba_sha256": black, "cases": results}, indent=2))

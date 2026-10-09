#!/usr/bin/env python3
"""Verify the preserved container/native evidence using exact rational times.

No native API is invoked. Invalid decoded DTS/durations and zero-sample
passthrough control records remain in the raw trace; they are not frames.
"""
from fractions import Fraction
import hashlib
import json
import math
from pathlib import Path
import sys

from verify_container import verify as verify_containers


def sha(data):
    return hashlib.sha256(data).hexdigest()


def rational(t):
    assert t["flags"] == 1 and t["epoch"] == 0 and t["timescale"] > 0
    return Fraction(t["value"], t["timescale"])


def verify_times(value, counts):
    if isinstance(value, dict):
        if {"value", "timescale", "epoch", "flags", "seconds"} <= value.keys():
            counts["total"] += 1
            if value["flags"] == 1:
                r = rational(value)
                assert math.isclose(value["seconds"], float(r), rel_tol=1e-15, abs_tol=1e-15)
                counts["numeric"] += 1
            else:
                assert value["flags"] in (0, 5) and value["seconds"] is None
                assert value["value"] == value["timescale"] == value["epoch"] == 0
                counts["invalid" if value["flags"] == 0 else "positive_infinity"] += 1
        else:
            for child in value.values():
                verify_times(child, counts)
    elif isinstance(value, list):
        for child in value:
            verify_times(child, counts)


def main(directory):
    containers = verify_containers(directory)["cases"]
    native = json.loads((directory / "native-read.json").read_text())
    encoder = json.loads((directory / "encoder-result.json").read_text())
    counts = dict(total=0, numeric=0, invalid=0, positive_infinity=0)
    verify_times(native, counts); verify_times(encoder, counts)
    assert len(native["cases"]) == 2
    assert encoder["sample_count"] == 3 and encoder["all_intra"] and not encoder["reordering"]
    for i, s in enumerate(encoder["samples"]):
        assert s["index"] == i and s["sync"]
        assert rational(s["input_pts"]) == rational(s["output_pts"]) == Fraction(i, 10)
        assert rational(s["output_dts"]) == Fraction(i, 10)
        assert rational(s["input_duration"]) == rational(s["output_duration"]) == Fraction(1, 10)
        assert sha((directory / "samples" / f"frame-{i}.avc").read_bytes()) == s["encoded_sha256"]
        rgba = (directory / "samples" / f"frame-{i}.rgba").read_bytes()
        assert len(rgba) == 64 * 48 * 4 and sha(rgba) == s["source_rgba_sha256"]
    before = json.loads((directory / "pre-read-originals.json").read_text())
    after = json.loads((directory / "post-read-originals.json").read_text())
    assert before["files"] == after["files"] and after["identical_to_pre_read"]
    assert all(f["mode"] == "0o444" for f in before["files"])
    summaries, decoded_pixels = [], []
    for c, raw in zip(containers, native["cases"]):
        assert c["file"] == raw["name"] + ".mov"
        assert c["sha256"] == raw["sha256_before"] == raw["sha256_between_reads"] == raw["sha256_after"]
        assert raw["source_unchanged"] and c["bytes"] == raw["file_bytes"]
        expected_pts = [Fraction(t, 600) for t in c["composition_ticks"]]
        expected_dts = [Fraction(t, 600) for t in c["decode_ticks"]]
        end = Fraction(c["movie_track_media_end_ticks"][0], 600)
        for mode in ("compressed", "decoded"):
            read = raw[mode]
            assert read["reader_status"] == 2 and read["started"] and read["reader_error"] is None
            assert read["default_time_range_unmodified"]
            request = read["requested_time_range"]
            assert rational(request["start"]) == 0 and request["duration"]["flags"] == 5
            assert rational(read["asset_duration"]) == rational(read["track_range"]["duration"]) == end
            assert rational(read["track_range"]["start"]) == 0
            assert len(read["segments"]) == 1 and not read["segments"][0]["empty"]
            segment = read["segments"][0]
            for coordinate in ("source", "target"):
                assert rational(segment[coordinate]["start"]) == 0
                assert rational(segment[coordinate]["duration"]) == end
            assert [r["ordinal"] for r in read["records"]] == list(range(len(read["records"])))
            for record in read["records"]:
                for key, hash_key in [("block_file", "block_sha256"), ("rgba_file", "rgba_sha256")]:
                    if key in record:
                        name = record[key]
                        assert Path(name).name == name
                        assert sha((directory / "reader-output" / name).read_bytes()) == record[hash_key]
        encoded = [r for r in raw["compressed"]["records"] if r["sample_count"] > 0]
        markers = [r for r in raw["compressed"]["records"] if r["sample_count"] == 0]
        assert len(encoded) == 3 and all(r["sample_count"] == 1 for r in encoded)
        assert all(not r["has_image"] for r in raw["compressed"]["records"])
        assert all(r["sample_bytes"] == 0 for r in markers)
        assert [r["block_sha256"] for r in encoded] == c["sample_sha256"]
        assert [r["sample_bytes"] for r in encoded] == c["sample_sizes"]
        assert [rational(r["pts"]) for r in encoded] == expected_pts
        assert [rational(r["dts"]) for r in encoded] == expected_dts
        assert [rational(r["duration"]) for r in encoded] == [Fraction(1, 10)] * 3
        frames = raw["decoded"]["records"]
        assert len(frames) == raw["decoded"]["decoded_image_count"] == 3
        assert all(r["has_image"] and r["sample_count"] == 1 for r in frames)
        assert [rational(r["pts"]) for r in frames] == expected_pts
        # AVAssetReader does not supply decoded DTS/duration for these frames.
        assert all(r["dts"]["flags"] == r["duration"]["flags"] == 0 for r in frames)
        pixels = [(directory / "reader-output" / r["rgba_file"]).read_bytes() for r in frames]
        assert all(len(p) == 64 * 48 * 4 for p in pixels)
        assert all(p != bytes([0, 0, 0, 255]) * (64 * 48) for p in pixels)
        decoded_pixels.append(pixels)
        summaries.append({"name": raw["name"], "movie_sha256": c["sha256"],
            "compressed_media_pts_exact": [str(rational(r["pts"])) for r in encoded],
            "compressed_media_dts_exact": [str(rational(r["dts"])) for r in encoded],
            "decoded_image_pts_exact": [str(rational(r["pts"])) for r in frames],
            "decoded_rgba_sha256": [r["rgba_sha256"] for r in frames],
            "passthrough_total_records": len(raw["compressed"]["records"]),
            "passthrough_media_samples": len(encoded), "passthrough_control_records_retained": len(markers),
            "first_actual_image_positive": rational(frames[0]["pts"]) > 0,
            "decoded_dts_and_duration": "invalid in original native trace; not synthesized",
            "default_full_range": "0 through positive infinity, unmodified",
            "both_readers_completed": True, "container_and_native_segments_have_no_empty_edit": True})
    assert decoded_pixels[0] == decoded_pixels[1]
    assert not summaries[0]["first_actual_image_positive"] and summaries[1]["first_actual_image_positive"]
    return {"native_positive_first_image_precondition_demonstrated": True,
        "cases": summaries, "rational_records_checked": counts,
        "decoded_pixel_bytes_identical_between_cases": True,
        "first_following_rust_seek_or_app_workflow_validated": False,
        "original_movies_and_timing_unchanged": True}


if __name__ == "__main__":
    print(json.dumps(main(Path(sys.argv[1])), indent=2, sort_keys=True))

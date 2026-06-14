#!/usr/bin/env python3
import argparse
import os
import sys

try:
    from ds_store import DSStore
    from mac_alias import Alias
except ImportError as exc:
    print(
        'Missing Python packages "ds_store" and "mac_alias". Install "dmgbuild" to provide them.',
        file=sys.stderr,
    )
    raise SystemExit(1) from exc


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mount-point", required=True)
    parser.add_argument("--app-name", required=True)
    parser.add_argument("--background-relative-path", required=True)
    parser.add_argument("--window-left", type=int, default=100)
    parser.add_argument("--window-top", type=int, default=100)
    parser.add_argument("--window-width", type=int, default=660)
    parser.add_argument("--window-height", type=int, default=422)
    parser.add_argument("--app-x", type=int, default=180)
    parser.add_argument("--app-y", type=int, default=172)
    parser.add_argument("--applications-x", type=int, default=480)
    parser.add_argument("--applications-y", type=int, default=172)
    parser.add_argument("--icon-size", type=float, default=180.0)
    parser.add_argument("--text-size", type=float, default=12.0)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    background_path = os.path.join(args.mount_point, args.background_relative_path)
    if not os.path.isfile(background_path):
        print(f"Background file not found: {background_path}", file=sys.stderr)
        return 1

    alias = Alias.for_file(background_path)
    ds_store_path = os.path.join(args.mount_point, ".DS_Store")
    window_bounds = "{{%d, %d}, {%d, %d}}" % (
        args.window_left,
        args.window_top,
        args.window_width,
        args.window_height,
    )

    bwsp = {
        "ContainerShowSidebar": True,
        "ShowPathbar": False,
        "ShowSidebar": True,
        "ShowStatusBar": False,
        "ShowTabView": False,
        "ShowToolbar": False,
        "SidebarWidth": 0,
        "WindowBounds": window_bounds,
    }
    icvp = {
        "arrangeBy": "none",
        "backgroundColorBlue": 1.0,
        "backgroundColorGreen": 1.0,
        "backgroundColorRed": 1.0,
        "backgroundImageAlias": alias.to_bytes(),
        "backgroundType": 2,
        "gridOffsetX": 0.0,
        "gridOffsetY": 0.0,
        "gridSpacing": 100.0,
        "iconSize": args.icon_size,
        "labelOnBottom": True,
        "scrollPositionX": 0.0,
        "scrollPositionY": 0.0,
        "showIconPreview": True,
        "showItemInfo": False,
        "textSize": args.text_size,
        "viewOptionsVersion": 1,
    }

    with DSStore.open(ds_store_path, "w+") as store:
        store["."]["vSrn"] = ("long", 1)
        store["."]["bwsp"] = bwsp
        store["."]["icvp"] = icvp
        store[args.app_name]["Iloc"] = (args.app_x, args.app_y)
        store["Applications"]["Iloc"] = (args.applications_x, args.applications_y)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())

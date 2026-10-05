#!/usr/bin/env python3

import argparse
from pathlib import Path


def depot_vdf(depot_id, local_path, depot_path="."):
    return f'''"DepotBuildConfig"
{{
    "DepotID" "{depot_id}"
    "FileMapping"
    {{
        "LocalPath" "{local_path}"
        "DepotPath" "{depot_path}"
        "recursive" "1"
    }}
}}
'''


def app_vdf(app_id, depots, content_root, description, branch):
    depot_lines = "\n".join(
        f'        "{did}" "{vdf}"' for did, vdf in depots
    )
    return f'''"appbuild"
{{
    "appid" "{app_id}"
    "desc" "{description}"
    "contentroot" "{content_root}"
    "buildoutput" "{content_root}/output"
    "setlive" "{branch}"
    "depots"
    {{
{depot_lines}
    }}
}}
'''


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--app-id", required=True)
    parser.add_argument("--branch", default="latest")
    parser.add_argument("--description", default="")
    parser.add_argument("--build-dir", required=True)
    args = parser.parse_args()

    app_id = int(args.app_id)
    build_dir = Path(args.build_dir).resolve()

    depots = [
        (app_id + 1, "./windows/*", "."),
        (app_id + 2, "./linux/*", "."),
        (app_id + 3, "./windows-webview2/*", "webview2-runtime"),
        (app_id + 4, "./linux-webview2/*", "webview2-runtime"),
    ]

    vdf_refs = []
    for depot_id, local_path, depot_path in depots:
        filename = f"depot_{depot_id}.vdf"
        (build_dir / filename).write_text(depot_vdf(depot_id, local_path, depot_path))
        vdf_refs.append((depot_id, filename))

    (build_dir / "output").mkdir(exist_ok=True)
    app_path = build_dir / "app_build.vdf"
    app_path.write_text(
        app_vdf(app_id, vdf_refs, str(build_dir), args.description, args.branch)
    )

    print(f"Generated {app_path} with {len(vdf_refs)} depots")


if __name__ == "__main__":
    main()

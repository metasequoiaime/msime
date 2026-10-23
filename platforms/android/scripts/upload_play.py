#!/usr/bin/env python3
"""Upload a signed Android App Bundle to a Google Play track.

One edit per run: upload the bundle, put its version code on the track as the only release there, commit. Play rejects a version code it has already seen, so a re-run of the same version fails at the upload rather than replacing what testers have.

The service account (a JSON key from Google Cloud) has to be invited in Play Console under Users and permissions with release rights on the app. Play Console only accepts a first bundle uploaded by hand; until the app has been reviewed and published once, a release can only be created as a draft (`--status draft`), which a person then rolls out from the Console.
"""
import argparse
import sys

from google.oauth2 import service_account
from googleapiclient.discovery import build
from googleapiclient.errors import HttpError
from googleapiclient.http import MediaFileUpload

SCOPE = "https://www.googleapis.com/auth/androidpublisher"
TRACKS = ("internal", "alpha", "beta", "production")
STATUSES = ("completed", "draft")


def upload(package: str, bundle: str, track: str, status: str, name: str, key_path: str) -> int:
    credentials = service_account.Credentials.from_service_account_file(key_path, scopes=[SCOPE])
    edits = build("androidpublisher", "v3", credentials=credentials, cache_discovery=False).edits()
    edit = edits.insert(packageName=package, body={}).execute()["id"]
    media = MediaFileUpload(bundle, mimetype="application/octet-stream", resumable=True)
    version_code = edits.bundles().upload(packageName=package, editId=edit, media_body=media).execute()["versionCode"]
    release = {"name": name, "versionCodes": [str(version_code)], "status": status}
    edits.tracks().update(packageName=package, editId=edit, track=track, body={"track": track, "releases": [release]}).execute()
    edits.commit(packageName=package, editId=edit).execute()
    return version_code


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--package", required=True, help="application id, e.g. app.msime.android")
    parser.add_argument("--bundle", required=True, help="signed .aab")
    parser.add_argument("--track", required=True, choices=TRACKS, help="alpha is the default closed testing track")
    parser.add_argument("--status", default="completed", choices=STATUSES)
    parser.add_argument("--name", required=True, help="release name shown in Play Console, e.g. 0.5.0")
    parser.add_argument("--key-path", required=True, help="service account JSON key")
    args = parser.parse_args()
    try:
        version_code = upload(args.package, args.bundle, args.track, args.status, args.name, args.key_path)
    except HttpError as error:
        # Play's own message says what is wrong (a version code already used, a draft-only app, a missing permission); the status line alone does not.
        print(f"Google Play rejected the upload: {error.status_code} {error.reason}\n{error.content.decode(errors='replace')}", file=sys.stderr)
        return 1
    print(f"{args.package} version code {version_code} is on the {args.track} track as {args.status}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

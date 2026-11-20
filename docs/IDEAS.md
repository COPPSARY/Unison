# Ideas

Features not marked "In" in spec §2 go here, not into the code.

- Copy a playlist Spotify → YouTube or back (v1.1).
- Local embedding model for ranking candidates (v2).

- Local files: "Remove from library" for a song whose file is missing outside watched folders (UP-12 keeps it greyed out with Locate file…).
- Extension streams (from EXT-05): DASH/HLS manifests (segments joined into one seekable in-memory source);
  an Opus decoder and WebM/Matroska demuxer so Opus qualities can play; passing user-entered extension
  settings and logins to `getDownloadUrl`.

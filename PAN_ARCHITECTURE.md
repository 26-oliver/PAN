# PAN desktop: Rust interface on VLC

## Product decision

PAN uses VLC's playback engine and media support from this fork. PAN's desktop UI and its application state are written in Rust and use one layout on macOS, Windows, and Linux. The VLC C/C++/Objective-C engine remains in the repository and is called through a small Rust FFI boundary. "All Rust" applies to PAN's new application layer, not a rewrite of VLC's engine.

The existing Qt and macOS VLC interfaces are not PAN's target UI. They remain in the fork while the Rust client reaches feature parity. PAN is a separate desktop binary built from this repository and linked to this fork's libVLC, not a second playback engine.

## Layout

```text
┌──────────────────┬─────────────────────────────────────────────┐
│ PAN              │ Search, open file, open URL, add to library │
│ Home             ├─────────────────────────────────────────────┤
│ Library          │                                             │
│  Music           │ Current page: library, playlist, search,   │
│  Videos          │ network browser, or video playback         │
│  Movies          │                                             │
│ Playlists        │                                             │
│ Internet         │                                             │
│ Settings         ├─────────────────────────────────────────────┤
│                  │ Now playing · timeline · transport · volume │
└──────────────────┴─────────────────────────────────────────────┘
```

The same navigation and control locations are used on all three platforms. Platform native file pickers and accessibility APIs may differ.

## Media rules

1. Open file, drag and drop, operating-system file association, and open URL immediately create a transient playback item. None of these actions adds a library entry or starts a folder scan.
2. Only an explicit **Add to library** action stores a file or folder. The user can remove a source without deleting the underlying files.
3. A library source is a user-selected local folder, mounted cloud folder, or an authenticated provider connector. A stream URL is playable without becoming a library source.
4. Playlists hold references to library items or explicit URLs. Playing an item never mutates the library.
5. Search distinguishes local library results from internet and service results. PAN does not claim to search arbitrary internet video sites without a supported provider or legal source API.

## Rust modules

- `pan-desktop`: cross-platform GUI, navigation, keyboard and accessibility behavior.
- `pan-playback`: safe Rust ownership wrapper around this fork's libVLC, playback queue, tracks, subtitles and events.
- `pan-library`: persistent opt-in sources, metadata index, playlists and search. Its database is separate from playback state.
- `pan-providers`: source adapters. Start with local paths and URLs; implement Google Drive using OAuth and its API as a named adapter, then other services individually.

The first executable milestone must open a local file and a network URL in an embedded video surface, show transport controls, and leave the library empty. The next milestone adds a chosen local folder and playlists. Cloud connectors follow after the local source lifecycle is reliable.

## Integration constraints

LibVLC 4 in this fork has a different media-player constructor from libVLC 3: `libvlc_media_player_new(instance, callbacks, opaque)`. Bindings must use the checked-in headers rather than assume libVLC 3 signatures. Video rendering uses libVLC output callbacks or a platform video surface owned by the Rust UI. Callback ownership, thread synchronization and shutdown are part of the playback wrapper.

Do not replace VLC's codec, demux, streaming or subtitle modules. PAN packaging must bundle or locate the matching libVLC and its plugins, retain required license notices, and be built and exercised separately on macOS, Windows and Linux.

## Acceptance checks

- Opening a downloaded file starts playback and the library remains unchanged.
- Choosing a folder adds only that folder as a library source; removing it does not delete files.
- A playlist can be created, renamed, played and reordered.
- A supported direct network URL starts playback without a library entry.
- The sidebar, search, video surface and controls work with keyboard, mouse and screen reader on all three systems.
- A missing or incompatible libVLC reports a useful error rather than silently falling back to another player.

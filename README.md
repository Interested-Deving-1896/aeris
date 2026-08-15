<div align="center">

[crates-shield]: https://img.shields.io/crates/v/aeris
[crates-url]: https://crates.io/crates/aeris
[release-shield]: https://img.shields.io/github/v/release/pkgforge/aeris?label=release
[release-url]: https://github.com/pkgforge/aeris/releases/latest
[downloads-shield]: https://img.shields.io/github/downloads/pkgforge/aeris/total?label=downloads
[downloads-url]: https://github.com/pkgforge/aeris/releases
[license-shield]: https://img.shields.io/github/license/pkgforge/aeris.svg
[license-url]: https://github.com/pkgforge/aeris/blob/main/LICENSE

<img src="assets/aeris.svg" alt="aeris" width="128" height="128">

# Aeris

[![Release][release-shield]][release-url]
[![Crates.io][crates-shield]][crates-url]
[![Downloads][downloads-shield]][downloads-url]
[![License: MIT][license-shield]][license-url]

**Manages your package managers.**

A desktop front end for the ones you already have,
built with Rust and [GPUI](https://gpui.rs).

</div>

## Overview

Most Linux systems end up with more than one package manager. Aeris does not
add another. It searches, installs, updates and removes through the ones
already installed, and shows the result as one list rather than several.

It never installs anything itself. Every operation is a command, described by
a TOML manifest that says what to run and how to read what comes back. Adding
a manager is writing a manifest rather than changing aeris.

[soar](https://github.com/pkgforge/soar) is built in and describes itself, so
aeris drives whichever soar is actually installed. Others are added at runtime
from the [adapter registry](https://github.com/pkgforge/aeris-registry).

## Features

- Search every enabled manager at once, ranked by how well each result answers
- Install, update and remove packages, and run what you installed
- Narrow a search to the managers you pick
- Work per user or system wide, for a manager that offers both
- Watch a manager work, in its own words, and answer it when it stops to ask
- See what a manager holds, what it can update, and what it cannot tell you
- Add adapters from the registry, refreshed on an interval and offered as updates
- Read as many registries as you like, your own included, in the order you trust them
- Declarative manifest view: edit `packages.toml`, preview the diff, and apply
- Per package detail panel with source, build, and option fields

## Install

### Portable binary (recommended)

Each release ships a single self-contained executable built with
[onelf](https://github.com/QaidVoid/onelf). It bundles its own libraries
and runs on most Linux systems without installing anything.

Download `aeris-x86_64-linux.onelf` from the
[latest release](https://github.com/pkgforge/aeris/releases/latest),
then:

```sh
chmod +x aeris-x86_64-linux.onelf
./aeris-x86_64-linux.onelf
```

Nightly builds are published on the rolling
[`nightly`](https://github.com/pkgforge/aeris/releases/tag/nightly) tag, and
are cut only when there is something new to build.

### From source

Requires a Rust toolchain and the usual GPUI build dependencies
(fontconfig, freetype, libxcb, libxkbcommon, wayland, and alsa headers).

```sh
cargo build --release
./target/release/aeris
```

A Nix flake is provided:

```sh
nix develop
```

## Adapters

An adapter is a TOML manifest naming the arguments for each operation and how
to read what comes back, so a manager that already answers in JSON needs
nothing more than a description. A manifest also says how the manager acts
system wide, which settings it accepts, and whether an operation needs a
terminal.

What a manager can do follows from what its manifest declares, and aeris
offers only that. A manager that cannot say which packages have updates is
asked to update everything at once rather than one package at a time, and says
so on the page rather than failing when pressed.

Manifests are read from, in order:

```
$XDG_DATA_HOME/aeris/adapters      # ~/.local/share/aeris/adapters
$XDG_DATA_DIRS/aeris/adapters      # /usr/local/share and /usr/share
./adapters
```

The first is where the Adapters page installs to, so a manifest you install
outranks one a package put there.

The Adapters page installs them from the registry and checks for newer ones.
See [pkgforge/aeris-registry](https://github.com/pkgforge/aeris-registry) for
the published manifests and the schema.

A manifest carries its own `version`, counting from one. It is the manifest
being versioned rather than the manager, so a manifest can be corrected
without waiting for a release, and a manager can release without every
manifest claiming to have changed. The Adapters page shows both: the version
of the manager it found, and the manifest revision driving it.

### Registries

A registry is a `registry.toml` listing manifests and their checksums, served
over HTTP(S) or read from a local path. Aeris ships knowing about
[the pkgforge one](https://github.com/pkgforge/aeris-registry) and reads it
when nothing else is configured.

Anything else you want to drive is a registry of your own. It can sit on a web
server, on a share, or in a directory on the machine, and needs no
coordination with pkgforge:

```toml
registries = [
  "https://raw.githubusercontent.com/pkgforge/aeris-registry/main/registry.toml",
  { name = "work", url = "https://packages.example.com/aeris/registry.toml" },
  "~/dev/my-adapters/registry.toml",
]
```

An entry is either a bare URL or a table naming it. A name is only what the
Settings page calls it; without one, aeris names it after where it is read
from. The order is the point rather than decoration: where two registries
offer the same adapter, the one listed first is the one offered, so putting
your own above the default is how you replace a published manifest.

The Settings page lists them. A row can be renamed, moved up or down, or
removed, and its Test button reads that registry once to report how many
adapters it offers or why it could not be read.

## Configuration

Aeris keeps its own settings in `~/.config/aeris/config.toml`, which the
Settings page writes. Every key is optional and the file need not exist.

```toml
theme = "system"               # system, light, dark
startup_view = "dashboard"     # dashboard, browse, installed, updates
notifications = true

# How long the copy of the registry on disk stays good for. Takes the words
# soar uses: always, never, auto, or a duration such as 30m, 3h, 1d.
registry_sync_interval = "3h"

disabled_adapters = ["pacstall"]

registries = [
  "https://raw.githubusercontent.com/pkgforge/aeris-registry/main/registry.toml",
]
```

Only aeris itself is configured here. A setting a manager owns is written to
that manager's own configuration, which is why the Settings page shows those
fields as they stand on disk until you override one.

## Contributing

Contributions are welcome. Please feel free to open issues or pull requests.

## License

MIT

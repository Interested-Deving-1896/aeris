# aeris

**Manages your package managers.** A desktop front end for the ones you
already have, built with Rust and [GPUI](https://gpui.rs).

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
~/.local/share/aeris/adapters
/usr/local/share/aeris/adapters
./adapters
```

The Adapters page installs them from the registry and checks for newer ones.
See [pkgforge/aeris-registry](https://github.com/pkgforge/aeris-registry) for
the published manifests and the schema.

## Contributing

Contributions are welcome. Please feel free to open issues or pull requests.

## License

MIT

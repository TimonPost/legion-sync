[![Donate](https://img.shields.io/badge/Donate-PayPal-green.svg)](https://www.paypal.com/cgi-bin/webscr?cmd=_s-xclick&hosted_button_id=Z8QK6XU749JB2) 
[![Latest Version][crate-badge]][crate-link] 
[![docs][docs-badge]][docs-link]
![Lines of Code][loc-badge]
[![MIT][license-badge]][license-link] 

# Development Note

There's a lot of construction work at the moment.
Don't use this library yet! Unless you want to contribute, of course. 
In terms of design there is nothing definitive, 
at the moment I'm still working on an API and I'm still in the research phase. 
 
See [examples](https://github.com/entity-sync-rs/legion-sync/tree/master/examples) for the operation of the API. 
I try to keep these up to date as much as possible. 
I also have a [base-game](https://github.com/entity-sync-rs/example-game) with which I develop this game. 
Feel free to ping me in the amethyst discord (Timon | Flying Dutchman#4256)

# Synchronize legion entities.
This library offers an abstraction on top of legion that can synchronize changing entities to other players.

## Features

- [X] Synchronize modified components.
    - [X] TCP-networking support.
    - [X] Tracks addition/removal/modification of components.     
- [X] Supports Custom compression.
- [X] Supports Custom serialisation.
- [X] Some resources, systems, components which makes entity synchronisation more easier.
- [X] Extra entity filters.

### Backlog
- State Model
- Deterministic Model
- Lockstep 
- Delta Encoding
- Reliable UDP support (laminar)
- Interest Management
- Snapshots
- Interpolation
- Client Side Perdition

# Examples

Upcomming...

[crate-badge]: https://img.shields.io/crates/v/legion-sync.svg
[crate-link]: https://crates.io/crates/legion-sync

[license-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[license-link]: ./docs/LICENSE

[docs-badge]: https://docs.rs/legion-sync/badge.svg
[docs-link]: https://docs.rs/legion-sync/

[loc-badge]: https://tokei.rs/b1/github/entity-sync-rs/legion-sync?category=code
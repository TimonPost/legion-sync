[![Donate](https://img.shields.io/badge/Donate-PayPal-green.svg)](https://www.paypal.com/cgi-bin/webscr?cmd=_s-xclick&hosted_button_id=Z8QK6XU749JB2) 
[![Latest Version][crate-badge]][crate-link] 
[![docs][docs-badge]][docs-link]
![Lines of Code][loc-badge]
[![MIT][license-badge]][license-link] 

# Synchronize legion entities.
This library offers an abstraction on top of legion that can synchronize changing entities to other players.

## Features

- [X] Synchronize modified components. 
- [X] Supports Custom compression.
- [X] Supports Custom serialisation.
- [X] Tracks addition/removal/modification of components.

# Examples

First, add `track` attribute to mark type as trackable.
```rust

```

[crate-badge]: https://img.shields.io/crates/v/legion-sync.svg
[crate-link]: https://crates.io/crates/legion-sync

[license-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[license-link]: ./docs/LICENSE

[docs-badge]: https://docs.rs/legion-sync/badge.svg
[docs-link]: https://docs.rs/legion-sync/

[loc-badge]: https://tokei.rs/b1/github/entity-sync-rs/legion-sync?category=code
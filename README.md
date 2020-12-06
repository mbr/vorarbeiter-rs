# Vorarbeiter, a small process supervisor

`vorarbeiter::Supervisor` shuts down processes it owns on `Drop` by sending a `SIGTERM` first, followed by a `SIGKILL`:

```rust
use std::process;

// The default kill timeout is 10 seconds, which is fine here.
let mut supervisor = vorarbeiter::Supervisor::default();

// Spawns three new child processes and adds them to the supervisor.
for _ in 0..3 {
    let child = process::Command::new("my-subcommand").spawn().unwrap();
    supervisor.add_child(child);
}

// Terminate all child processes.
drop(supervisor);
```

See the [documentation](https://docs.rs/vorarbeiter) for details.

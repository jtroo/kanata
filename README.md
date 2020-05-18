# ktrl
PoC of QMK for Linux written in Rust

## Initial Design

### KbdIn

```
strcut KbdIn {
  device: evdev_rs::Device;
};

impl KbdIn {
    new(dev_path: std::path::Path) -> Self
    read_key(&mut self) -> Result<InputEvent>
}
```

### KbdOut

```
strcut KbdOut {
  device: std::fs::File,
};

impl KbdOut {
    new() -> Self
    write_key(event: &InputEvent) -> Result<()>
    write_key_press(event: &InputEvent) -> Result<()>
    write_key_release(event: &InputEvent) -> Result<()>
}
```

### Ktrl

```
struct Ktrl {
  in: KbdIn,
  out: KbdOut,
}

impl Ktrl {
    new(in: KbdIn, out: KbdOut) -> Self
    event_loop(&mut self) -> Result<()>
}
```

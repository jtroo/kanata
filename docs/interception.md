# Windows Interception driver implementation notes

- Interception handle is `!Send` and `!Sync`
  - means a single thread should own both input and output
  - `KbdOut` will need to send keyboard output events to that thread as opposed
    to Linux using `uinput` and the original Windows code using `SendInput`
    which are independent of the input devices.
  - Maybe save channel in kanata struct as part of new kanata
- Interception can filter for only keyboard events
  - should use this filter feature; don't want to intercept mouse
- Need to save previous device for sending to, in case wait/receive (with
  timeout) don't return anything so that sending stuff can be sent to some
  device.
- Input `ScanCode` maps to the keyberon `KeyCode`; they both use the USB
  standard codes.
  - For ease of integration will probably need to unfortunately convert it to
    an `OsCode` even though the processing loop will soon after just convert it
    back to `KeyCode`. Oh well.

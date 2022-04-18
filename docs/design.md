# Design doc

## main

- read args
- read config
- start event loops

## event loop

- read key events
- send events to processing loop on mpsc

## processing loop

- check for events on mpsc
- if event: send event to layout
- tick() the keyberon layout, send any events needed
- if no event: sleep for 7ms (~144 Hz)

## layout

- uses keyberon
- indices of `keyberon::layout::Event::{Press, Release}(x,y)`:

    x = 0   # keyberon doesn't handle values larger than 0
    y = keycode % 256

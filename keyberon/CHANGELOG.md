# v0.2.0

* New Keyboard::leds_mut function for getting underlying leds object.
* Made Layout::current_layer public for getting current active layer.
* Added a procedural macro for defining layouts (`keyberon::layout::layout`)
* Corrected HID report descriptor
* Add max_packet_size() to HidDevice to allow differing report sizes
* Allows default layer to be set on a Layout externally
* Add Chording for multiple keys pressed at the same time to equal another key

Breaking changes:
* Row and Column pins are now a simple array. For the STM32 MCU, you
  should now use `.downgrade()` to have an homogenous array. 
* `Action::HoldTap` now takes a configuration for different behaviors.
* `Action::HoldTap` now takes the `tap_hold_interval` field. Not
  implemented yet.
* `Action` is now generic, for the `Action::Custom(T)` variant,
  allowing custom actions to be handled outside of keyberon. This
  functionality can be used to drive non keyboard actions, such as resetting
  the microcontroller, driving leds (for backlight or underglow for
  example), managing a mouse emulation, or any other ideas you can
  have. As there is a default value for the type parameter, the update
  should be transparent.
* Layers don't sum anymore, the last pressed layer action set the layer.
* Rename MeidaCoffee in MediaCoffee to fix typo.

# v0.1.1

*  HidClass::control_xxx: check interface number [#26](https://github.com/TeXitoi/keyberon/pull/26)

# v0.1.0

First published version.

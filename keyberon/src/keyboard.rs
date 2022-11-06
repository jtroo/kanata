//! Keyboard HID device implementation.

use crate::hid::{self, HidDevice, Protocol, ReportType, Subclass};
use crate::key_code::KbHidReport;

/// A trait to manage keyboard LEDs.
///
/// `()` implements this trait if you don't care of LEDs.
pub trait Leds {
    /// Sets the num lock state.
    fn num_lock(&mut self, _status: bool) {}
    /// Sets the caps lock state.
    fn caps_lock(&mut self, _status: bool) {}
    /// Sets the scroll lock state.
    fn scroll_lock(&mut self, _status: bool) {}
    /// Sets the compose state.
    fn compose(&mut self, _status: bool) {}
    /// Sets the kana state.
    fn kana(&mut self, _status: bool) {}
}
impl Leds for () {}

#[rustfmt::skip]
const REPORT_DESCRIPTOR: &[u8] = &[
    0x05, 0x01,        // Usage Page (Generic Desktop Ctrls)
    0x09, 0x06,        // Usage (Keyboard)
    0xA1, 0x01,        // Collection (Application)
    0x05, 0x07,        //   Usage Page (Kbrd/Keypad)
    0x19, 0xE0,        //   Usage Minimum (0xE0)
    0x29, 0xE7,        //   Usage Maximum (0xE7)
    0x15, 0x00,        //   Logical Minimum (0)
    0x25, 0x01,        //   Logical Maximum (1)
    0x95, 0x08,        //   Report Count (8)
    0x75, 0x01,        //   Report Size (1)
    0x81, 0x02,        //   Input (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position)
    0x95, 0x01,        //   Report Count (1)
    0x75, 0x08,        //   Report Size (8)
    0x81, 0x03,        //   Input (Const,Var,Abs,No Wrap,Linear,Preferred State,No Null Position)
    0x05, 0x07,        //   Usage Page (Kbrd/Keypad)
    0x19, 0x00,        //   Usage Minimum (0x00)
    0x29, 0xFF,        //   Usage Maximum (0xFF)
    0x15, 0x00,        //   Logical Minimum (0)
    0x26, 0xFF, 0x00,  //   Logical Maximum (255)
    0x95, 0x06,        //   Report Count (6)
    0x75, 0x08,        //   Report Size (8)
    0x81, 0x00,        //   Input (Data,Array,Abs,No Wrap,Linear,Preferred State,No Null Position)
    0x05, 0x08,        //   Usage Page (LEDs)
    0x19, 0x01,        //   Usage Minimum (Num Lock)
    0x29, 0x05,        //   Usage Maximum (Kana)
    0x95, 0x05,        //   Report Count (5)
    0x75, 0x01,        //   Report Size (1)
    0x91, 0x02,        //   Output (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
    0x95, 0x01,        //   Report Count (1)
    0x75, 0x03,        //   Report Size (3)
    0x91, 0x03,        //   Output (Const,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
    0xC0,              // End Collection
];

/// A keyboard HID device.
pub struct Keyboard<L> {
    report: KbHidReport,
    leds: L,
}

impl<L> Keyboard<L> {
    /// Creates a new `Keyboard` object.
    pub fn new(leds: L) -> Keyboard<L> {
        Keyboard {
            report: KbHidReport::default(),
            leds,
        }
    }
    /// Set the current keyboard HID report.  Returns `true` if it is modified.
    pub fn set_keyboard_report(&mut self, report: KbHidReport) -> bool {
        if report == self.report {
            false
        } else {
            self.report = report;
            true
        }
    }

    /// Returns the underlying leds object.
    pub fn leds_mut(&mut self) -> &mut L {
        &mut self.leds
    }
}

impl<L: Leds> HidDevice for Keyboard<L> {
    fn subclass(&self) -> Subclass {
        Subclass::BootInterface
    }

    fn protocol(&self) -> Protocol {
        Protocol::Keyboard
    }

    fn max_packet_size(&self) -> u16 {
        8
    }

    fn report_descriptor(&self) -> &[u8] {
        REPORT_DESCRIPTOR
    }

    fn get_report(&mut self, report_type: ReportType, _report_id: u8) -> Result<&[u8], hid::Error> {
        match report_type {
            ReportType::Input => Ok(self.report.as_bytes()),
            _ => Err(hid::Error),
        }
    }

    fn set_report(
        &mut self,
        report_type: ReportType,
        report_id: u8,
        data: &[u8],
    ) -> Result<(), hid::Error> {
        if report_type == ReportType::Output && report_id == 0 && data.len() == 1 {
            let d = data[0];
            self.leds.num_lock(d & 1 != 0);
            self.leds.caps_lock(d & 1 << 1 != 0);
            self.leds.scroll_lock(d & 1 << 2 != 0);
            self.leds.compose(d & 1 << 3 != 0);
            self.leds.kana(d & 1 << 4 != 0);
            return Ok(());
        }
        Err(hid::Error)
    }
}

// Copyright 2019 Robin Krahl <robin.krahl@ireas.org>, Guillaume Pinot <texitoi@texitoi.eu>
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(missing_docs)]

use usb_device::bus::{InterfaceNumber, StringIndex, UsbBus, UsbBusAllocator};
use usb_device::class::{ControlIn, ControlOut, UsbClass};
use usb_device::control;
use usb_device::control::{Recipient, RequestType};
use usb_device::descriptor::DescriptorWriter;
use usb_device::endpoint::{EndpointAddress, EndpointIn};
use usb_device::UsbError;

const SPECIFICATION_RELEASE: u16 = 0x111;
const INTERFACE_CLASS_HID: u8 = 0x03;

pub struct Error;

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(u8)]
pub enum Subclass {
    None = 0x00,
    BootInterface = 0x01,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(u8)]
pub enum Protocol {
    None = 0x00,
    Keyboard = 0x01,
    Mouse = 0x02,
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum DescriptorType {
    Hid = 0x21,
    Report = 0x22,
    _Physical = 0x23,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum Request {
    GetReport = 0x01,
    GetIdle = 0x02,
    GetProtocol = 0x03,
    SetReport = 0x09,
    SetIdle = 0x0a,
    SetProtocol = 0x0b,
}
impl Request {
    fn new(u: u8) -> Option<Request> {
        use Request::*;
        match u {
            0x01 => Some(GetReport),
            0x02 => Some(GetIdle),
            0x03 => Some(GetProtocol),
            0x09 => Some(SetReport),
            0x0a => Some(SetIdle),
            0x0b => Some(SetProtocol),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReportType {
    Input,
    Output,
    Feature,
    Reserved(u8),
}

impl From<u8> for ReportType {
    fn from(val: u8) -> Self {
        match val {
            1 => ReportType::Input,
            2 => ReportType::Output,
            3 => ReportType::Feature,
            _ => ReportType::Reserved(val),
        }
    }
}

pub trait HidDevice {
    fn subclass(&self) -> Subclass;

    fn protocol(&self) -> Protocol;

    fn max_packet_size(&self) -> u16;

    fn report_descriptor(&self) -> &[u8];

    fn set_report(
        &mut self,
        report_type: ReportType,
        report_id: u8,
        data: &[u8],
    ) -> Result<(), Error>;

    fn get_report(&mut self, report_type: ReportType, report_id: u8) -> Result<&[u8], Error>;
}

pub struct HidClass<'a, B: UsbBus, D: HidDevice> {
    device: D,
    interface: InterfaceNumber,
    endpoint_interrupt_in: EndpointIn<'a, B>,
    expect_interrupt_in_complete: bool,
}

impl<B: UsbBus, D: HidDevice> HidClass<'_, B, D> {
    pub fn new(device: D, alloc: &UsbBusAllocator<B>) -> HidClass<'_, B, D> {
        let max_packet_size = device.max_packet_size();
        HidClass {
            device,
            interface: alloc.interface(),
            endpoint_interrupt_in: alloc.interrupt(max_packet_size, 10),
            expect_interrupt_in_complete: false,
        }
    }

    pub fn device_mut(&mut self) -> &mut D {
        &mut self.device
    }

    pub fn write(&mut self, data: &[u8]) -> Result<usize, Error> {
        if self.expect_interrupt_in_complete {
            return Ok(0);
        }

        if data.len() >= 8 {
            self.expect_interrupt_in_complete = true;
        }

        match self.endpoint_interrupt_in.write(data) {
            Ok(count) => Ok(count),
            Err(UsbError::WouldBlock) => Ok(0),
            Err(_) => Err(Error),
        }
    }

    fn get_report(&mut self, xfer: ControlIn<B>) {
        let req = xfer.request();
        let [report_type, report_id] = req.value.to_be_bytes();
        let report_type = ReportType::from(report_type);
        match self.device.get_report(report_type, report_id) {
            Ok(data) => xfer.accept_with(data).ok(),
            Err(Error) => xfer.reject().ok(),
        };
    }

    fn set_report(&mut self, xfer: ControlOut<B>) {
        let req = xfer.request();
        let [report_type, report_id] = req.value.to_be_bytes();
        let report_type = ReportType::from(report_type);
        match self.device.set_report(report_type, report_id, xfer.data()) {
            Ok(()) => xfer.accept().ok(),
            Err(Error) => xfer.reject().ok(),
        };
    }

    fn interface_index(&self) -> u16 {
        let iface: u8 = self.interface.into();
        iface as u16
    }
}

impl<B: UsbBus, D: HidDevice> UsbClass<B> for HidClass<'_, B, D> {
    fn poll(&mut self) {}

    fn reset(&mut self) {
        self.expect_interrupt_in_complete = false;
    }

    fn get_configuration_descriptors(
        &self,
        writer: &mut DescriptorWriter,
    ) -> usb_device::Result<()> {
        writer.interface(
            self.interface,
            INTERFACE_CLASS_HID,
            self.device.subclass() as u8,
            self.device.protocol() as u8,
        )?;

        let report_descriptor = self.device.report_descriptor();
        let descriptor_len = report_descriptor.len();
        if descriptor_len > u16::max_value() as usize {
            return Err(UsbError::InvalidState);
        }
        let descriptor_len = (descriptor_len as u16).to_le_bytes();
        let specification_release = SPECIFICATION_RELEASE.to_le_bytes();
        writer.write(
            DescriptorType::Hid as u8,
            &[
                specification_release[0],     // bcdHID.lower
                specification_release[1],     // bcdHID.upper
                0,                            // bCountryCode: 0 = not supported
                1,                            // bNumDescriptors
                DescriptorType::Report as u8, // bDescriptorType
                descriptor_len[0],            // bDescriptorLength.lower
                descriptor_len[1],            // bDescriptorLength.upper
            ],
        )?;

        writer.endpoint(&self.endpoint_interrupt_in)?;

        Ok(())
    }

    fn get_string(&self, _index: StringIndex, _lang_id: u16) -> Option<&str> {
        None
    }

    fn endpoint_in_complete(&mut self, addr: EndpointAddress) {
        if addr == self.endpoint_interrupt_in.address() {
            self.expect_interrupt_in_complete = false;
        }
    }

    fn endpoint_out(&mut self, _addr: EndpointAddress) {}

    fn control_in(&mut self, xfer: ControlIn<B>) {
        let req = xfer.request();
        match (req.request_type, req.recipient) {
            (RequestType::Standard, Recipient::Interface) => {
                if req.request == control::Request::GET_DESCRIPTOR {
                    let (dtype, index) = req.descriptor_type_index();
                    if dtype == DescriptorType::Report as u8
                        && index == 0
                        && req.index == self.interface_index()
                    {
                        let descriptor = self.device.report_descriptor();
                        xfer.accept_with(descriptor).ok();
                    }
                }
            }
            (RequestType::Class, Recipient::Interface) => {
                if let Some(request) = Request::new(req.request) {
                    if request == Request::GetReport && req.index == self.interface_index() {
                        self.get_report(xfer);
                    }
                }
            }
            _ => {}
        }
    }

    fn control_out(&mut self, xfer: ControlOut<B>) {
        let req = xfer.request();
        if req.request_type == RequestType::Class && req.recipient == Recipient::Interface {
            if let Some(request) = Request::new(req.request) {
                if request == Request::SetReport && req.index == self.interface_index() {
                    self.set_report(xfer);
                }
            }
        }
    }
}

// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use pci::{
    PciClassCode, PciConfiguration, PciDevice, PciDeviceError, PciHeaderType, PciInterruptPin,
    PciProgrammingInterface, PciSerialBusSubClass,
};
use resources::SystemAllocator;
use std::os::unix::io::RawFd;
use std::sync::Arc;
use sys_util::{EventFd, GuestMemory};
use usb::host_backend::host_backend_device_provider::HostBackendDeviceProvider;
use usb::xhci::intr_resample_handler::IntrResampleHandler;
use usb::xhci::mmio_space::MMIOSpace;
use usb::xhci::xhci::Xhci;
use usb::xhci::xhci_backend_device_provider::XhciBackendDeviceProvider;
use usb::xhci::xhci_regs::init_xhci_mmio_space_and_regs;

const XHCI_BAR0_SIZE: u64 = 0x10000;

#[derive(Clone, Copy)]
enum UsbControllerProgrammingInterface {
    Usb3HostController = 0x30,
}

impl PciProgrammingInterface for UsbControllerProgrammingInterface {
    fn get_register_value(&self) -> u8 {
        *self as u8
    }
}

/// xHCI PCI interface implementation.
pub struct XhciController {
    config_regs: PciConfiguration,
    mem: GuestMemory,
    bar0: u64, // bar0 in config_regs will be changed by guest. Not sure why.
    device_provider: Option<HostBackendDeviceProvider>,
    irq_evt: Option<EventFd>,
    irq_resample_evt: Option<EventFd>,
    mmio: Option<MMIOSpace>,
    xhci: Option<Arc<Xhci>>,
}

impl XhciController {
    pub fn new(mem: GuestMemory, usb_provider: HostBackendDeviceProvider) -> Self {
        let config_regs = PciConfiguration::new(
            0x01b73, // fresco logic, (google = 0x1ae0)
            0x1000,  // fresco logic pdk. This chip has broken msi. See kernel xhci-pci.c
            PciClassCode::SerialBusController,
            &PciSerialBusSubClass::USB,
            Some(&UsbControllerProgrammingInterface::Usb3HostController),
            PciHeaderType::Device,
            0,
            0,
        );
        XhciController {
            config_regs,
            bar0: 0,
            device_provider: Some(usb_provider),
            irq_evt: None,
            irq_resample_evt: None,
            mmio: None,
            mem,
            xhci: None,
        }
    }

    pub fn init_when_forked(&mut self) {
        if self.mmio.is_some() {
            error!("xhci controller is already inited");
            return;
        }
        let (mmio, regs) = init_xhci_mmio_space_and_regs();
        self.mmio = Some(mmio);
        self.xhci = Some(Xhci::new(
            self.mem.clone(),
            self.device_provider.take().unwrap(),
            self.irq_evt.take().unwrap(),
            self.irq_resample_evt.take().unwrap(),
            regs,
        ));
    }
}

impl PciDevice for XhciController {
    fn keep_fds(&self) -> Vec<RawFd> {
        let raw_fd = self.device_provider.as_ref().unwrap().keep_fds();
        vec![raw_fd]
    }
    fn assign_irq(
        &mut self,
        irq_evt: EventFd,
        irq_resample_evt: EventFd,
        irq_num: u32,
        irq_pin: PciInterruptPin,
    ) {
        self.config_regs.set_irq(irq_num as u8, irq_pin);
        self.irq_evt = Some(irq_evt);
        self.irq_resample_evt = Some(irq_resample_evt);
    }

    fn allocate_io_bars(
        &mut self,
        resources: &mut SystemAllocator,
    ) -> Result<Vec<(u64, u64)>, PciDeviceError> {
        // xHCI spec 5.2.1.
        let bar0 = resources
            .allocate_mmio_addresses(XHCI_BAR0_SIZE)
            .ok_or(PciDeviceError::IoAllocationFailed(XHCI_BAR0_SIZE))?;
        self.config_regs
            .add_memory_region(bar0, XHCI_BAR0_SIZE)
            .ok_or(PciDeviceError::IoRegistrationFailed(bar0))?;
        self.bar0 = bar0;
        Ok(vec![(bar0, XHCI_BAR0_SIZE)])
    }

    fn config_registers(&self) -> &PciConfiguration {
        &self.config_regs
    }

    fn config_registers_mut(&mut self) -> &mut PciConfiguration {
        let bar0 = self.config_regs.get_bar_addr(0) as u64;
        let reg = self.config_regs.read_reg(2);
        &mut self.config_regs
    }

    fn read_bar(&mut self, addr: u64, data: &mut [u8]) {
        let bar0 = self.bar0;
        if addr < bar0 || addr > bar0 + XHCI_BAR0_SIZE {
            return;
        }
        self.mmio.as_ref().unwrap().read_bar(addr - bar0, data);
    }

    fn write_bar(&mut self, addr: u64, data: &[u8]) {
        let bar0 = self.bar0;
        if addr < bar0 || addr > bar0 + XHCI_BAR0_SIZE {
            return;
        }
        self.mmio.as_ref().unwrap().write_bar(addr - bar0, data);
    }
    fn on_device_sandboxed(&mut self) {
        self.init_when_forked();
    }
}

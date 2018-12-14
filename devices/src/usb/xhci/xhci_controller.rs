// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use pci::{
    PciClassCode, PciConfiguration, PciDevice, PciDeviceError, PciHeaderType, PciInterruptPin,
    PciProgrammingInterface, PciSerialBusSubClass,
};
use resources::SystemAllocator;
use std::mem;
use std::os::unix::io::RawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use sys_util::{EventFd, GuestMemory};
use usb::host_backend::host_backend_device_provider::HostBackendDeviceProvider;
use usb::xhci::mmio_register::Register;
use usb::xhci::mmio_space::MMIOSpace;
use usb::xhci::xhci::Xhci;
use usb::xhci::xhci_backend_device_provider::XhciBackendDeviceProvider;
use usb::xhci::xhci_regs::{init_xhci_mmio_space_and_regs, XhciRegs};

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

/// Use this handle to fail xhci controller.
pub struct XhciFailHandle {
    usbcmd: Register<u32>,
    usbsts: Register<u32>,
    xhci_failed: AtomicBool,
}

impl XhciFailHandle {
    pub fn new(regs: &XhciRegs) -> XhciFailHandle {
        XhciFailHandle {
            usbcmd: regs.usbcmd.clone(),
            usbsts: regs.usbsts.clone(),
            xhci_failed: AtomicBool::new(false),
        }
    }

    /// Fail this controller. Will set related registers and flip failed bool.
    pub fn fail(&self) {
        // set run/stop to stop.
        const USBCMD_STOPPED: u32 = 0;
        // Set host system error bit.
        const USBSTS_HSE: u32 = 1 << 2;
        self.usbcmd.set_value(USBCMD_STOPPED);
        self.usbsts.set_value(USBSTS_HSE);

        self.xhci_failed.store(true, Ordering::SeqCst);
        error!("xhci controller stopped working");
    }

    /// Returns true if xhci is already failed.
    pub fn failed(&self) -> bool {
        self.xhci_failed.load(Ordering::SeqCst)
    }
}

// Xhci controller should be created with backend device provider. Then irq should be assigned
// before initialized. We are not making `failed` as a state here to optimize performance. Cause we
// need to set failed in other threads.
enum XhciControllerState {
    Unknown,
    Created {
        device_provider: HostBackendDeviceProvider,
    },
    IrqAssigned {
        device_provider: HostBackendDeviceProvider,
        irq_evt: EventFd,
        irq_resample_evt: EventFd,
    },
    Initialized {
        mmio: MMIOSpace,
        xhci: Arc<Xhci>,
        fail_handle: Arc<XhciFailHandle>,
    },
}

/// xHCI PCI interface implementation.
pub struct XhciController {
    config_regs: PciConfiguration,
    mem: GuestMemory,
    bar0: u64, // bar0 in config_regs will be changed by guest. Not sure why.
    state: XhciControllerState,
}

impl XhciController {
    /// Create new xhci controller.
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
            mem,
            bar0: 0,
            state: XhciControllerState::Created {
                device_provider: usb_provider,
            },
        }
    }

    /// Init xhci controller when it's forked.
    pub fn init_when_forked(&mut self) {
        match mem::replace(&mut self.state, XhciControllerState::Unknown) {
            XhciControllerState::IrqAssigned {
                device_provider,
                irq_evt,
                irq_resample_evt,
            } => {
                let (mmio, regs) = init_xhci_mmio_space_and_regs();
                let fail_handle = Arc::new(XhciFailHandle::new(&regs));
                self.state = XhciControllerState::Initialized {
                    mmio,
                    xhci: Xhci::new(
                        self.mem.clone(),
                        device_provider,
                        irq_evt,
                        irq_resample_evt,
                        regs,
                    ),
                    fail_handle,
                }
            }
            _ => {
                error!("xhci controller is in a wrong state");
                panic!();
            }
        }
    }
}

impl PciDevice for XhciController {
    fn keep_fds(&self) -> Vec<RawFd> {
        match self.state {
            XhciControllerState::Created {
                ref device_provider,
            } => device_provider.keep_fds(),
            _ => {
                error!("xhci controller is in a wrong state");
                panic!();
            }
        }
    }

    fn assign_irq(
        &mut self,
        irq_evt: EventFd,
        irq_resample_evt: EventFd,
        irq_num: u32,
        irq_pin: PciInterruptPin,
    ) {
        match mem::replace(&mut self.state, XhciControllerState::Unknown) {
            XhciControllerState::Created { device_provider } => {
                self.config_regs.set_irq(irq_num as u8, irq_pin);
                self.state = XhciControllerState::IrqAssigned {
                    device_provider,
                    irq_evt,
                    irq_resample_evt,
                }
            }
            _ => {
                error!("xhci controller is in a wrong state");
                panic!();
            }
        }
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
        &mut self.config_regs
    }

    fn read_bar(&mut self, addr: u64, data: &mut [u8]) {
        let bar0 = self.bar0;
        if addr < bar0 || addr > bar0 + XHCI_BAR0_SIZE {
            return;
        }
        match self.state {
            XhciControllerState::Initialized {
                ref mmio,
                xhci: _,
                fail_handle: _,
            } => {
                // Read bar would still work even if it's already failed.
                mmio.read_bar(addr - bar0, data);
            }
            _ => {
                error!("xhci controller is in a wrong state");
                panic!();
            }
        }
    }

    fn write_bar(&mut self, addr: u64, data: &[u8]) {
        let bar0 = self.bar0;
        if addr < bar0 || addr > bar0 + XHCI_BAR0_SIZE {
            return;
        }
        match self.state {
            XhciControllerState::Initialized {
                ref mmio,
                xhci: _,
                ref fail_handle,
            } => {
                if !fail_handle.failed() {
                    mmio.write_bar(addr - bar0, data);
                }
            }
            _ => {
                error!("xhci controller is in a wrong state");
                panic!();
            }
        }
    }
    fn on_device_sandboxed(&mut self) {
        self.init_when_forked();
    }
}
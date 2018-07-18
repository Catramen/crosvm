// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use super::mmio_space::MMIOSpace;
use super::mmio_register::{
    BarOffset,
    BarRange,
    Register,
    RegisterInterface,
    RegisterSpec,
    StaticRegister,
    StaticRegisterSpec,
};

const XHCI_CAPLENGTH: u8 = 0x20;
const XHCI_DBOFF: u32 = 0x00002000;
const XHCI_RTSOFF: u32 = 0x00003000;

// Bitmasks for the usbcmd register.
const USB_CMD_RUNSTOP: u32 = 1u32 << 0;
const USB_CMD_RESET: u32 = 1u32 << 1;
const USB_CMD_INTERRUPTER_ENABLE: u32 = 1u32 << 2;

// Bitmasks for the usbsts register.
const USB_STS_HALTED: u32 = 1u32 << 0;
const USB_STS_EVENT_INTERRUPT: u32 = 1u32 << 3;
const USB_STS_PORT_CHANGE_DETECT: u32 = 1u32 << 4;
const USB_STS_CONTROLLER_NOT_READY: u32 = 1u32 << 11;
const USB_STS_SET_TO_CLEAR_MASK: u32 = 0x0000041C;

// Bitmasks for the crcr register.
const CRCR_RING_CYCLE_STATE: u64 = 1u64 << 0;
const CRCR_COMMAND_STOP: u64 = 1u64 << 1;
const CRCR_COMMAND_ABORT: u64 = 1u64 << 2;
const CRCR_COMMAND_RING_RUNNING: u64 = 1u64 << 3;
const CRCR_COMMAND_RING_POINTER: u64 = 0xFFFFFFFFFFFFFFC0;

// Bitmasks for portsc registers.
const PORTSC_CURRENT_CONNECT_STATUS: u32 = 1u32 << 0;
const PORTSC_PORT_ENABLED: u32 = 1u32 << 1;
const PORTSC_PORT_RESET: u32 = 1u32 << 4;
const PORTSC_PORT_LINK_STATE_MASK: u32 = 0x000001E0;
const PORTSC_PORT_POWER: u32 = 1u32 << 9;
const PORTSC_CONNECT_STATUS_CHANGE: u32 = 1u32 << 17;
const PORTSC_PORT_ENABLED_DISABLED_CHANGE: u32 = 1u32 << 18;
const PORTSC_PORT_RESET_CHANGE: u32 = 1u32 << 21;
const PORTSC_WARM_PORT_RESET: u32 = 1u32 << 31;
const PORTSC_SET_TO_CLEAR_MASK: u32 = 0x00FE0002;

// Bitmasks for iman registers.
const IMAN_INTERRUPT_PENDING: u32 = 1u32 << 0;
const IMAN_INTERRUPT_ENABLE: u32 = 1u32 << 1;
const IMAN_SET_TO_CLEAR_MASK: u32 = 0x00000001;

// Bitmasks and offsets for imod registers.
const IMOD_INTERRUPT_MODERATION_INTERVAL: u32 = 0xFFFF;
const IMOD_INTERRUPT_MODERATION_COUNTER_OFFSET: u8 = 16;

// Bitmasks for erstsz registers.
const ERSTSZ_SEGMENT_TABLE_SIZE: u32 = 0xFFFF;

// Bitmasks for erstba registers.
const ERSTBA_SEGMENT_TABLE_BASE_ADDRESS: u64 = 0xFFFFFFFFFFFFFFC0;

// Bitmasks for erdp registers.
const ERDP_EVENT_HANDLER_BUSY: u64 = 1u64 << 3;
const ERDP_EVENT_RING_DEQUEUE_POINTER: u64 = 0xFFFFFFFFFFFFFFF0;
const ERDP_SET_TO_CLEAR_MASK: u64 = 0x0000000000000008;

// Bitmasks and offsets for doorbell registers.
const DOORBELL_TARGET: u32 = 0xFF;
const DOORBELL_STREAM_ID_OFFSET: u32 = 16;

// Bitmasks and offsets for structural parameter registers.
const HCSPARAMS1_MAX_INTERRUPTERS_MASK: u32 = 0x7FF00;
const HCSPARAMS1_MAX_INTERRUPTERS_OFFSET: u32 = 8;
const HCSPARAMS1_MAX_SLOTS_MASK: u32 = 0xFF;

// Bitmasks and offsets for extended capabilities registers.
const SPCAP_PORT_COUNT_MASK: u32 = 0xFF00;
const SPCAP_PORT_COUNT_OFFSET: u32 = 8;

pub struct XHCIRegs {
    pub usbcmd: Register<u32>,
    pub usbsts: Register<u32>,
    pub dnctrl: Register<u32>,
    pub crcr: Register<u64>,
    pub dcbaap: Register<u64>,
    pub config: Register<u64>,
    pub portsc: Vec<Register<u32>>,
    pub doorbells: Vec<Register<u32>>,
    pub iman: Vec<Register<u32>>,
    pub imod: Vec<Register<u32>>,
    pub erstsz: Vec<Register<u32>>,
    pub erstba: Vec<Register<u64>>,
    pub erdp: Vec<Register<u64>>,
}

/// This function returns mmio space definition for xhci. See Xhci spec chapter 5
/// for details.
pub fn init_xhci_mmio_space_and_regs() -> (MMIOSpace, XHCIRegs) {
    let mut mmio = MMIOSpace::new();
    /**************************************************************************/

    /***************** Host Controller Capability Registers *******************/
    mmio.add_register(
        // CAPLENGTH
        static_register!(
            ty: u8,
            offset: 0x00,
            value: XHCI_CAPLENGTH, // Operation register start at offset 0x20
            ),
    );
    mmio.add_register(
        // HCIVERSION
        static_register!(
            ty: u16,
            offset: 0x02,
            value: 0x0110,// Revision 1.1
            ),
    );
    mmio.add_register(
        // HCSPARAMS1
        static_register!(
            ty: u32,
            offset: 0x04,
            value: 0x08000108, // max_slots = 8, max_interrupters = 1, max_ports = 8
            ),
    );

    mmio.add_register(
        // HCSPARAMS2
        static_register!(
            ty: u32,
            offset: 0x08,
            // Maximum number of event ring segment table entries = 32k
            // No scratchpad buffers.
            value: 0xf0,
            ),
    );

    mmio.add_register(
        // HCSPARAM3
        static_register!(
            ty: u32,
            offset: 0x0c,

            // Exit latencies for U1 (standby with fast exit) and U2 (standby with
            // slower exit) power states. We use the max values:
            // - U1 to U0: < 10 us
            // - U2 to U1: < 2047 us
            value: 0x07FF000A,
            ),
    );

    mmio.add_register(
        // HCCPARAMS1
        static_register!(
            ty: u32,
            offset: 0x10,
            // Supports 64 bit addressing
            // Max primary stream array size = 0 (streams not supported).
            // Extended capabilities pointer = 0xC000 offset from base.
            value: 0x30000501,
            ),
    );
    mmio.add_register(
        // DBOFF
        static_register!(
            ty: u32,
            offset: 0x14,
            value: XHCI_DBOFF, // Doorbell array offset 0x2000 from base.
            ),
    );

    mmio.add_register(
        // RTSOFF
        static_register!(
            ty: u32,
            offset: 0x18,
            value: XHCI_RTSOFF, // Runtime registers offset 0x3000 from base.
            ),
    );

    mmio.add_register(
        // HCCPARAMS2
        static_register!(
            ty: u32,
            offset: 0x1c,
            value: 0,
            ),
    );
    /************** End of Host Controller Capability Registers ***************/
    /**************************************************************************/

    /**************************************************************************/
    /***************** Host Controller Operational Registers ******************/
    let usbcmd = register!(
            ty: u32,
            offset: 0x20,
            reset_value: 0,
            guest_writeable_mask: 0x00002F0F,
            guest_write_1_to_clear_mask: 0,
        );
    mmio.add_register(usbcmd.clone());

    let usbsts = register!(
            ty: u32,
            offset: 0x24,
            reset_value: 0x00000001,
            guest_writeable_mask: 0x0000041C,
            guest_write_1_to_clear_mask: 0x0000041C,
        );
    mmio.add_register(usbsts.clone());

    mmio.add_register(
        //  Pagesize
        static_register!(
            ty: u32,
            offset: 0x28,
            value: 0x00000001,
            ),
    );

    let dnctrl = register!(
            ty: u32,
            offset: 0x34,
            reset_value: 0,
            guest_writeable_mask: 0x0000FFFF,
            guest_write_1_to_clear_mask: 0,
        );
    mmio.add_register(dnctrl.clone());

    let crcr = register!(
            ty: u64,
            offset: 0x38,
            reset_value: 9,
            guest_writeable_mask: 0xFFFFFFFFFFFFFFC7,
            guest_write_1_to_clear_mask: 0,
        );
    mmio.add_register(crcr.clone());

    let dcbaap = register!(
            ty: u64,
            offset: 0x50,
            reset_value: 0x0,
            guest_writeable_mask: 0xFFFFFFFFFFFFFFC0,
            guest_write_1_to_clear_mask: 0,
        );
    mmio.add_register(dcbaap.clone());

    let config = register!(
            ty: u64,
            offset: 0x58,
            reset_value: 0,
            guest_writeable_mask: 0x0000003F,
            guest_write_1_to_clear_mask: 0,
        );
    mmio.add_register(config.clone());

    let portsc = register_array!(
        ty: u32,
        cnt: 8, //  Must be equal to max_ports
        base_offset: 0x420,
        stride: 16,
        reset_value: 0x000002A0,
        guest_writeable_mask: 0x8EFFC3F2,
        guest_write_1_to_clear_mask: 0x00FE0002,);
    mmio.add_register_array(&portsc);

    // Portpmsc.
    mmio.add_register_array(&register_array!(
            ty: u32,
            cnt: 8,
            base_offset: 0x424,
            stride: 16,
            reset_value: 0,
            guest_writeable_mask: 0x0001FFFF,
            guest_write_1_to_clear_mask: 0,));

    // Portli
    mmio.add_register_array(&register_array!(
            ty: u32,
            cnt: 8,
            base_offset: 0x428,
            stride: 16,
            reset_value: 0,
            guest_writeable_mask: 0,
            guest_write_1_to_clear_mask: 0,));

    // Porthlpmc
    mmio.add_register_array(&register_array!(
            ty: u32,
            cnt: 8,
            base_offset: 0x42c,
            stride: 16,
            reset_value: 0,
            guest_writeable_mask: 0x00003FFF,
            guest_write_1_to_clear_mask: 0,));

    let doorbells = register_array!(
        ty: u32,
        cnt: 9, //  Must be equal to max_ports
        base_offset: 0x2000,
        stride: 4,
        reset_value: 0,
        guest_writeable_mask: 0xFFFF00FF,
        guest_write_1_to_clear_mask: 0,);
    mmio.add_register_array(&doorbells);

    /**************************************************************************/
    /***************************** Runtime Registers **************************/

    mmio.add_register(
        // mfindex
        static_register!(
            ty: u32,
            offset: 0x3000,
            value: 0, // 4 ports starting at port 5
            ),
    );

    /*************************** Reg Array for interrupters *******************/
    let iman = register_array!(
        ty: u32,
        cnt: 1, //  Must be equal to max_ports
        base_offset: 0x3020,
        stride: 32,
        reset_value: 0,
        guest_writeable_mask: 0x00000003,
        guest_write_1_to_clear_mask: 0x00000001,);
    mmio.add_register_array(&iman);

    let imod = register_array!(
        ty: u32,
        cnt: 1, //  Must be equal to max_ports
        base_offset: 0x3024,
        stride: 32,
        reset_value: 0x00000FA0,
        guest_writeable_mask: 0xFFFFFFFF,
        guest_write_1_to_clear_mask: 0,);
    mmio.add_register_array(&imod);

    let erstsz = register_array!(
        ty: u32,
        cnt: 1, //  Must be equal to max_ports
        base_offset: 0x3028,
        stride: 32,
        reset_value: 0,
        guest_writeable_mask: 0x0000FFFF,
        guest_write_1_to_clear_mask: 0,);
    mmio.add_register_array(&erstsz);

    let erstba = register_array!(
        ty: u64,
        cnt: 1, //  Must be equal to max_ports
        base_offset: 0x3030,
        stride: 32,
        reset_value: 0,
        guest_writeable_mask: 0xFFFFFFFFFFFFFFC0,
        guest_write_1_to_clear_mask: 0,);
    mmio.add_register_array(&erstba);

    let erdp = register_array!(
        ty: u64,
        cnt: 1, //  Must be equal to max_ports
        base_offset: 0x3038,
        stride: 32,
        reset_value: 0,
        guest_writeable_mask: 0xFFFFFFFFFFFFFFFF,
        guest_write_1_to_clear_mask: 0x0000000000000008,);
    mmio.add_register_array(&erdp);

    /************************* End of Runtime Registers ***********************/
    /**************************************************************************/

    let xhci_regs =  XHCIRegs {
        usbcmd: usbcmd,
        usbsts: usbsts,
        dnctrl: dnctrl,
        crcr: crcr,
        dcbaap: dcbaap,
        config: config,
        portsc: portsc,
        doorbells: doorbells,
        iman: iman,
        imod: imod,
        erstsz: erstsz,
        erstba: erstba,
        erdp: erdp,
    };


    /************** End of Host Controller Operational Registers **************/
    /**************************************************************************/

    /**************************************************************************/
    /********************** Extended Capability Registers *********************/

    // Extended capability registers. Base offset defined by hccparams1.
    // Each set of 4 registers represents a "Supported Protocol" extended
    // capability.  The first capability indicates that ports 1-4 are USB 2.0 and
    // the second capability indicates that ports 5-8 are USB 3.0.
    mmio.add_register(
        // spcap 1.1
        static_register!(
            ty: u32,
            offset: 0xc000,
            // "Supported Protocol" capability.
            // Next capability at 0x40 dwords offset.
            // USB 2.0.
            value: 0x20,
            ),
    );
    mmio.add_register(
        // spcap 1.2
        static_register!(
            ty: u32,
            offset: 0xc004,
            value: 0x20425355, // Name string = "USB "
            ),
    );
    mmio.add_register(
        // spcap 1.3
        static_register!(
            ty: u32,
            offset: 0xc008,
            value: 0x00000401, // 4 ports starting at port 1.
            ),
    );

    mmio.add_register(
        // spcap 1.4
        static_register!(
            ty: u32,
            offset: 0xc00c,
            // The specification says that this shall be set to 0 with no explanation.
            // Section 7.2.2.1.4.
            value: 0,
            ),
    );

    mmio.add_register(
        // spcap 2.1
        static_register!(
            ty: u32,
            offset: 0xc100,
            // "Supported Protocol" capability.
            // No pointer to next capability.
            // USB 3.0.
            value: 0x03000002,
            ),
    );

    mmio.add_register(
        // spcap 2.2
        static_register!(
            ty: u32,
            offset: 0xc104,
            value: 0x20425355, // Name string = "USB "
            ),
    );

    mmio.add_register(
        // spcap 2.3
        static_register!(
            ty: u32,
            offset: 0xc108,
            value: 0x00000405, // 4 ports starting at port 5
            ),
    );

    mmio.add_register(
        // spcap 2.4
        static_register!(
            ty: u32,
            offset: 0xc10c,
            // The specification says that this shall be set to 0 with no explanation.
            // Section 7.2.2.1.4.
            value: 0,
            ),
    );
    /************** End of Host Controller Operational Registers **************/
    /**************************************************************************/

    (mmio, xhci_regs)
}

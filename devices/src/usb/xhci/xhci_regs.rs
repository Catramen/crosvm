// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

const XHCI_CAPLENGTH: u64 = 0x20;
const XHCI_DBOFF: u64 = 0x00002000;
const XHCI_RTSOFF: u64 = 0x00003000;

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

struct XHCIRegs {
    usbcmd: &'static Register,
    usbsts: &'static Register,
    dnctrl: &'static Register,
    crcr: &'static Register,
    dcbaap: &'static Register,
    config: &'static Register,
    portsc: Vec<&'static Register>,
    doorbells: Vec<&'static Register>,
    iman: Vec<&'static Register>,
    imod: Vec<&'static Register>,
    erstsz: Vec<&'static Register>,
    erstba: Vec<&'static Register>,
    erdp: Vec<&'static Register>,
}

// This function returns mmio space definition for xhci. See Xhci spec chapter 5
// for details.
pub fn get_xhci_mmio_space_and_regs() -> (MMIOSpace, XHCIRegs) {
    let mut mmio = MMIO::new();
    /**************************************************************************/

    /***************** Host Controller Capability Registers *******************/
    mmio.add_reg(
        // CAPLENGTH
        register!(
            offset: 0x00,
            size: 1,
            reset_value: XHCI_CAPLENGTH, // Operation register start at offset 0x20
            ),
    );
    mmio.add_reg(
        // HCIVERSION
        register!(
            offset: 0x02,
            size: 2,
            reset_value: 0x0110,// Revision 1.1
            ),
    );
    mmio.add_reg(
        // HCSPARAMS1
        register!(
            offset: 0x04,
            size: 4,
            reset_value: 0x08000108, // max_slots = 8, max_interrupters = 1, max_ports = 8
            ),
    );

    mmio.add_reg(
        // HCSPARAMS2
        register!(
            offset: 0x08,
            size: 4,
            // Maximum number of event ring segment table entries = 32k
            // No scratchpad buffers.
            reset_value: 0xf0,
            ),
    );

    mmio.add_reg(
        // HCSPARAM3
        register!(
            offset: 0x0c,
            size: 4,

            // Exit latencies for U1 (standby with fast exit) and U2 (standby with
            // slower exit) power states. We use the max values:
            // - U1 to U0: < 10 us
            // - U2 to U1: < 2047 us
            reset_value: 0x07FF000A,
            ),
    );

    mmio.add_reg(
        // HCCPARAMS1
        register!(
            offset: 0x10,
            size: 4,

            // Supports 64 bit addressing
            // Max primary stream array size = 0 (streams not supported).
            // Extended capabilities pointer = 0xC000 offset from base.
            reset_value: 0x30000501
            ),
    );
    mmio.add_reg(
        // DBOFF
        register!(
            offset: 0x14,
            size: 4,
            reset_value: XHCI_DBOFF, // Doorbell array offset 0x2000 from base.
            ),
    );

    mmio.add_reg(
        // RTSOFF
        register!(
            offset: 0x18,
            size: 4,
            reset_value: XHCI_RTSOFF, // Runtime registers offset 0x3000 from base.
            ),
    );

    mmio.add_reg(
        // HCCPARAMS2
        register!(
            offset: 0x1c,
            size: 4,
            reset_value: 0,
            ),
    );
    /************** End of Host Controller Capability Registers ***************/
    /**************************************************************************/

    /**************************************************************************/
    /***************** Host Controller Operational Registers ******************/
    let usbcmd = register!(
            offset: 0x20,
            size: 4,
            reset_value: 0,
            guest_writeable_mask: 0x00002F0F,
            guest_write_1_to_clear_mask: 0,
        );

    let usbsts = register!(
            offset: 0x24,
            size: 4,
            reset_value: 0x00000001,
            guest_writeable_mask: 0x0000041C,
            guest_write_1_to_clear_mask: 0x0000041C,
        );

    mmio.add_reg(
        //  Pagesize
        register!(
            offset: 0x28,
            size: 4,
            reset_value: 0x00000001,
            ),
    );

    let dnctrl = register!(
            offset: 0x34,
            size: 4,
            reset_value: 0,
            guest_writeable_mask: 0x0000FFFF,
            guest_write_1_to_clear_mask: 0,
        );

    let crcr = register!(
            offset: 0x38,
            size: 8,
            reset_value: 9,
            guest_writeable_mask: 0xFFFFFFFFFFFFFFC7,
            guest_write_1_to_clear_mask: 0,
        );

    let dcbaap = register!(
            offset: 0x50,
            size: 8,
            reset_value: 0x0,
            guest_writeable_mask: 0xFFFFFFFFFFFFFFC0,
            guest_write_1_to_clear_mask: 0,
        );

    let config = register!(
            offset: 0x58,
            size: 8,
            reset_value: 0,
            guest_writeable_mask: 0x0000003F,
            guest_write_1_to_clear_mask: 0,
        );

    let portsc = register_array!(cnt: 8, //  Must be equal to max_ports
                                 base_offset: 0x420,
                                 stride: 16,
                                 size: 4,
                                 reset_value: 0x000002A0,
                                 guest_writeable_mask: 0x8EFFC3F2,
                                 guest_write_1_to_clear_mask: 0x00FE0002,);

    // Portpmsc.
    mmio.add_reg_array(register_array!(cnt: 8,
                                       base_offset: 0x424,
                                       stride: 16,
                                       size: 4,
                                       reset_value: 0,
                                       guest_writeable_mask: 0x0001FFFF,
                                       guest_write_1_to_clear_mask: 0,));

    // Portli
    mmio.add_reg_array(register_array!(cnt: 8,
                                       base_offset: 0x428,
                                       stride: 16,
                                       size: 4,
                                       reset_value: 0,
                                       guest_writeable_mask: 0,
                                       guest_write_1_to_clear_mask: 0,));

    // Porthlpmc
    mmio.add_reg_array(register_array!(cnt 8,
                                       base_offset: 0x42c,
                                       stride: 16,
                                       size: 4,
                                       reset_value: 0
                                       guest_writeable_mask: 0x00003FFF,
                                       guest_write_1_to_clear_mask: 0,));

    let doorbells = register_array!(cnt: 9, //  Must be equal to max_ports
                                    base_offset: 0x2000,
                                    size: 4,
                                    reset_value: 0,
                                    guest_writeable_mask: 0xFFFF00FF,
                                    guest_write_1_to_clear_mask: 0,);

    /**************************************************************************/
    /***************************** Runtime Registers **************************/

    mmio.add_reg(
        // mfindex
        register!(
            offset: 0x3000,
            size: 4,
            reset_value: 0, // 4 ports starting at port 5
            ),
    );


    /*************************** Reg Array for interrupters *******************/
    let iman = register_array!(cnt: 1, //  Must be equal to max_ports
                               base_offset: 0x3020,
                               stride: 32,
                               size: 4,
                               reset_value: 0,
                               guest_writeable_mask: 0x00000003,
                               guest_write_1_to_clear_mask: 0x00000001,);

    let imod = register_array!(cnt: 1, //  Must be equal to max_ports
                               base_offset: 0x3024,
                               stride: 32,
                               size: 4,
                               reset_value: 0x00000FA0,
                               guest_writeable_mask: 0xFFFFFFFF,
                               guest_write_1_to_clear_mask: 0,);



    let erstsz = register_array!(cnt: 1, //  Must be equal to max_ports
                                 base_offset: 0x3028,
                                 stride: 32,
                                 size: 4,
                                 reset_value: 0,
                                 guest_writeable_mask: 0x0000FFFF,
                                 guest_write_1_to_clear_mask: 0,);



    let erstba = register_array!(cnt: 1, //  Must be equal to max_ports
                                 base_offset: 0x3030,
                                 stride: 32,
                                 size: 8,
                                 reset_value: 0,
                                 guest_writeable_mask: 0xFFFFFFFFFFFFFFC0,
                                 guest_write_1_to_clear_mask: 0,);



    let erdp = register_array!(cnt: 1, //  Must be equal to max_ports
                               base_offset: 0x3038,
                               stride: 32,
                               size: 8,
                               reset_value: 0,
                               guest_writeable_mask: 0xFFFFFFFFFFFFFFFF,
                               guest_write_1_to_clear_mask: 0x0000000000000008);


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
    mmio.add_reg(
        // spcap 1.1
        register!(
            offset: 0xc000,
            size: 4,
            // "Supported Protocol" capability.
            // Next capability at 0x40 dwords offset.
            // USB 2.0.
            reset_value: 0x20,
            ),
    );
    mmio.add_reg(
        // spcap 1.2
        register!(
            offset: 0xc004,
            size: 4,
            reset_value: 0x20425355, // Name string = "USB "
            ),
    );
    mmio.add_reg(
        // spcap 1.3
        register!(
            offset: 0xc008,
            size: 4,
            reset_value: 0x00000401, // 4 ports starting at port 1.
            ),
    );

    mmio.add_reg(
        // spcap 1.4
        register!(
            offset: 0xc00c,
            size: 4,
            // The specification says that this shall be set to 0 with no explanation.
            // Section 7.2.2.1.4.
            reset_value: 0,
            ),
    );

    mmio.add_reg(
        // spcap 2.1
        register!(
            offset: 0xc100,
            size: 4,
            // "Supported Protocol" capability.
            // No pointer to next capability.
            // USB 3.0.
            reset_value: 0x03000002,
            ),
    );

    mmio.add_reg(
        // spcap 2.2
        register!(
            offset: 0xc104,
            size: 4,
            reset_value: 0x20425355, // Name string = "USB "
            ),
    );

    mmio.add_reg(
        // spcap 2.3
        register!(
            offset: 0xc108,
            size: 4,
            reset_value: 0x00000405, // 4 ports starting at port 5
            ),
    );

    mmio.add_reg(
        // spcap 2.4
        register!(
            offset: 0xc10c,
            size: 4,
            // The specification says that this shall be set to 0 with no explanation.
            // Section 7.2.2.1.4.
            reset_value: 0,
            ),
    );
    /************** End of Host Controller Operational Registers **************/
    /**************************************************************************/

    (mmio, xhci_regs)
}

// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

const XHCI_CAPLENGTH: u64 = 0x20;
const XHCI_DBOFF: u64 = 0x00002000;
const XHCI_RTSOFF: u64 = 0x00003000;


fn add_reg_array<C: RegisterCallback>(base: BarOffset, stride: BarOffset,
                                      reg_template: Register, callback: C) {
}

// This function returns mmio space definition for xhci. See Xhci spec chapter 5
// for details.
pub fn get_xhci_mmio_space(xhci_state: Rc<RefCell<XhciState>>) -> MMIOSpace {
    let mut mmio = MMIO::new();
    /**************************************************************************/ 
    /***************** Host Controller Capability Registers *******************/
    mmio.add_reg(
        // CAPLENGTH
        Register::new_ro(
            0x00, // bar offset
            1,    // size
            XHCI_CAPLENGTH, // Operation register start at offset 0x20
            )
        );
    mmio.add_reg(
        // HCIVERSION
        Register::new_ro(
            0x02,  // bar offset
            2,     // size
            0x0110,// Revision 1.1
            )
        );
    mmio.add_reg(
        // HCSPARAMS1
        Register::new_ro(
            0x04, // bar offset
            4,    // size
            0x08000108, // max_slots = 8, max_interrupters = 1, max_ports = 8
            )
        );

    mmio.add_reg(
        // HCSPARAMS2
        Register::new_ro(
            0x08, // bar offset
            4,    // size
            // Maximum number of event ring segment table entries = 32k
            // No scratchpad buffers.
            0xf0,
            )
        );

    mmio.add_reg(
        // HCSPARAM3
        Register::new_ro(
            0x0c, // bar offset
            4,    // size

            // Exit latencies for U1 (standby with fast exit) and U2 (standby with
            // slower exit) power states. We use the max values:
            // - U1 to U0: < 10 us
            // - U2 to U1: < 2047 us
            0x07FF000A,
            )
        );

    mmio.add_reg(
        // HCCPARAMS1
        Register::new_ro(
            0x10, // bar offset
            4,    // size

            // Supports 64 bit addressing
            // Max primary stream array size = 0 (streams not supported).
            // Extended capabilities pointer = 0xC000 offset from base.
            0x30000501
            )
        );
    mmio.add_reg(
        // DBOFF
        Register::new_ro(
            0x14, // bar offset
            4,    // size
            XHCI_DBOFF, // Doorbell array offset 0x2000 from base.
            )
        );

    mmio.add_reg(
        // RTSOFF
        Register::new_ro(
            0x18, // bar offset
            4,    // size
            XHCI_RTSOFF, // Runtime registers offset 0x3000 from base.
            )
        );

    mmio.add_reg(
        // HCCPARAMS2
        Register::new_ro(
            0x1c, // bar offset
            4,    // size
            0,
            )
        );
    /************** End of Host Controller Capability Registers ***************/
    /**************************************************************************/ 

    /**************************************************************************/ 
    /***************** Host Controller Operational Registers ******************/
    mmio.add_reg(
        Register::new_ro(
            0x18, // bar offset
            4,    // size
            0x20, //
            )
        );
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
        Register::new_ro(
            0xc000, // bar offset
            4,    // size
            // "Supported Protocol" capability.
            // Next capability at 0x40 dwords offset.
            // USB 2.0.
            0x20,
            )
        );
    mmio.add_reg(
        // spcap 1.2
        Register::new_ro(
            0xc004, // bar offset
            4,    // size
            0x20425355, // Name string = "USB "
            )
        );
    mmio.add_reg(
        // spcap 1.3
        Register::new_ro(
            0xc008, // bar offset
            4,    // size
            0x00000401, // 4 ports starting at port 1.
            )
        );

    mmio.add_reg(
        // spcap 1.4
        Register::new_ro(
            0xc00c, // bar offset
            4,    // size
            // The specification says that this shall be set to 0 with no explanation.
            // Section 7.2.2.1.4.
            0,
            )
        );

    mmio.add_reg(
        // spcap 2.1
        Register::new_ro(
            0xc100, // bar offset
            4,    // size
            // "Supported Protocol" capability.
            // No pointer to next capability.
            // USB 3.0.
            0x03000002,
            )
        );

    mmio.add_reg(
        // spcap 2.2
        Register::new_ro(
            0xc104, // bar offset
            4,    // size
            0x20425355, // Name string = "USB "
            )
        );

    mmio.add_reg(
        // spcap 2.3
        Register::new_ro(
            0xc108, // bar offset
            4,    // size
            0x00000405, // 4 ports starting at port 5
            )
        );

    mmio.add_reg(
        // spcap 2.4
        Register::new_ro(
            0xc10c, // bar offset
            4,    // size
            // The specification says that this shall be set to 0 with no explanation.
            // Section 7.2.2.1.4.
            0,
            )
        );
    /************** End of Host Controller Operational Registers **************/
    /**************************************************************************/
}


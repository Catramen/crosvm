// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

const XHCI_CAPLENGTH: u64 = 0x20;
const XHCI_DBOFF: u64 = 0x00002000;
const XHCI_RTSOFF: u64 = 0x00003000;

// This function returns mmio space definition for xhci. See Xhci spec chapter 5
// for details.
pub fn get_xhci_mmio_space(xhci_state: Rc<RefCell<XhciState>>) -> MMIOSpace {
    let mut mmio = MMIO::new();
    /**************************************************************************/ 
    /***************** Host Controller Capability Registers *******************/
    mmio.add_reg(
        // CAPLENGTH
        register!(
            offset: 0x00,
            size: 1,    
            reset_value: XHCI_CAPLENGTH, // Operation register start at offset 0x20
            )
        );
    mmio.add_reg(
        // HCIVERSION
        register!(
            offset: 0x02,
            size: 2,     
            reset_value: 0x0110,// Revision 1.1
            )
        );
    mmio.add_reg(
        // HCSPARAMS1
        register!(
            offset: 0x04,
            size: 4,    
            reset_value: 0x08000108, // max_slots = 8, max_interrupters = 1, max_ports = 8
            )
        );

    mmio.add_reg(
        // HCSPARAMS2
        register!(
            offset: 0x08,
            size: 4,    
            // Maximum number of event ring segment table entries = 32k
            // No scratchpad buffers.
            reset_value: 0xf0,
            )
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
            )
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
            )
        );
    mmio.add_reg(
        // DBOFF
        register!(
            offset: 0x14,
            size: 4,    
            reset_value: XHCI_DBOFF, // Doorbell array offset 0x2000 from base.
            )
        );

    mmio.add_reg(
        // RTSOFF
        register!(
            offset: 0x18,
            size: 4,    
            reset_value: XHCI_RTSOFF, // Runtime registers offset 0x3000 from base.
            )
        );

    mmio.add_reg(
        // HCCPARAMS2
        register!(
            offset: 0x1c,
            size: 4,    
            reset_value: 0,
            )
        );
    /************** End of Host Controller Capability Registers ***************/
    /**************************************************************************/ 

    /**************************************************************************/ 
    /***************** Host Controller Operational Registers ******************/
    mmio.add_reg(
        register!(
            offset: 0x18,
            size: 4,    
            reset_value: 0x20, //
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
        register!(
            offset: 0xc000,
            size: 4,    
            // "Supported Protocol" capability.
            // Next capability at 0x40 dwords offset.
            // USB 2.0.
            reset_value: 0x20,
            )
        );
    mmio.add_reg(
        // spcap 1.2
        register!(
            offset: 0xc004,
            size: 4,    
            reset_value: 0x20425355, // Name string = "USB "
            )
        );
    mmio.add_reg(
        // spcap 1.3
        register!(
            offset: 0xc008,
            size: 4,    
            reset_value: 0x00000401, // 4 ports starting at port 1.
            )
        );

    mmio.add_reg(
        // spcap 1.4
        register!(
            offset: 0xc00c,
            size: 4,    
            // The specification says that this shall be set to 0 with no explanation.
            // Section 7.2.2.1.4.
            reset_value: 0,
            )
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
            )
        );

    mmio.add_reg(
        // spcap 2.2
        register!(
            offset: 0xc104,
            size: 4,    
            reset_value: 0x20425355, // Name string = "USB "
            )
        );

    mmio.add_reg(
        // spcap 2.3
        register!(
            offset: 0xc108,
            size: 4,    
            reset_value: 0x00000405, // 4 ports starting at port 5
            )
        );

    mmio.add_reg(
        // spcap 2.4
        register!(
            offset: 0xc10c,
            size: 4,    
            // The specification says that this shall be set to 0 with no explanation.
            // Section 7.2.2.1.4.
            reset_value: 0,
            )
        );
    /************** End of Host Controller Operational Registers **************/
    /**************************************************************************/
}


// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include <stdint.h>

// This file includes the structs defined in xHCI specification.
// Rust does not have native support for bitfield, so this is in c.
// Note all the structs are little endian. It is default for bindgen and our
// target.

struct Trb {
  // ':64' seems unnecessary here. It's here to work around some bindgen bug:
  // https://github.com/rust-lang-nursery/rust-bindgen/issues/1314
  uint64_t parameter : 64;
  uint32_t status : 32;
  uint8_t cycle : 1;
  uint16_t flags : 9;
  uint8_t type : 6;
  uint16_t control : 16;
} __attribute__((__packed__));

struct NormalTrb {
  uint64_t data_buffer_lo : 64;
  uint32_t trb_transfer_length : 17;
  uint8_t td_size : 5;
  uint16_t interrupter_target : 10;
  uint8_t cycle : 1;
  uint8_t evaluate_next_trb : 1;
  uint8_t interrupt_on_short_packet : 1;
  uint8_t no_snoop : 1;
  uint8_t chain : 1;
  uint8_t interrupt_on_completion : 1;
  uint8_t immediate_data : 1;
  uint8_t reserved : 2;
  uint8_t block_event_interrupt : 1;
  uint8_t type : 6;
  uint16_t reserved1 : 16;
} __attribute__((__packed__));


struct SetupStageTrb : public TrbBase {
  uint8 request_type;
  uint8 request;
  uint16 value;
  uint16 index;
  uint16 length;
  uint32 trb_transfer_length : 17;
  uint8 reserved0 : 5;
  uint16 interrupter_target : 10;
  uint8 cycle : 1;
  uint8 reserved1 : 4;
  uint8 interrupt_on_completion : 1;
  uint8 immediate_data : 1;
  uint8 reserved2 : 3;
  uint8 type : 6;
  uint8 transfer_type : 2;
  uint16 reserved3 : 14;
} ABSL_ATTRIBUTE_PACKED;
static_assert(sizeof(SetupStageTrb) == kTrbSize,
              "SetupStageTrb has incorrect size.");

struct DataStageTrb : public TrbBase {
  uint64 data_buffer_pointer;
  uint32 trb_transfer_length : 17;
  uint8 td_size : 5;
  uint16 interrupter_target : 10;
  uint8 cycle : 1;
  uint8 evaluate_next_trb : 1;
  uint8 interrupt_on_short_packet : 1;
  uint8 no_snoop : 1;
  uint8 chain : 1;
  uint8 interrupt_on_completion : 1;
  uint8 immediate_data : 1;
  uint8 reserved0 : 3;
  uint8 type : 6;
  uint8 direction : 1;
  uint16 reserved1 : 15;
} ABSL_ATTRIBUTE_PACKED;
static_assert(sizeof(DataStageTrb) == kTrbSize,
              "DataStageTrb has incorrect size.");

struct StatusStageTrb : public TrbBase {
  uint64 reserved0;
  uint32 reserved1 : 22;
  uint16 interrupter_target : 10;
  uint8 cycle : 1;
  uint8 evaluate_next_trb : 1;
  uint8 reserved2 : 2;
  uint8 chain : 1;
  uint8 interrupt_on_completion : 1;
  uint8 reserved3 : 4;
  uint8 type : 6;
  uint8 direction : 1;
  uint16 reserved4 : 15;
} ABSL_ATTRIBUTE_PACKED;
static_assert(sizeof(StatusStageTrb) == kTrbSize,
              "StatusStageTrb has incorrect size.");

struct IsochTrb : public TrbBase {
  uint64 data_buffer_pointer;
  uint32 trb_transfer_length : 17;
  uint8 td_size : 5;
  uint16 interrupter_target : 10;
  uint8 cycle : 1;
  uint8 evaulate_nex_trb : 1;
  uint8 interrupt_on_short_packet : 1;
  uint8 no_snoop : 1;
  uint8 chain : 1;
  uint8 interrupt_on_completion : 1;
  uint8 immediate_data : 1;
  uint8 transfer_burst_count : 2;
  uint8 block_event_interrupt : 1;
  uint8 type : 6;
  uint8 tlbpc : 4;
  uint16 frame_id : 11;
  uint8 sia : 1;
} ABSL_ATTRIBUTE_PACKED;
static_assert(sizeof(IsochTrb) == kTrbSize, "IsochTrb has incorrect size.");

struct LinkTrb : public TrbBase {
  uint64 ring_segment_pointer;
  uint32 reserved0 : 22;
  uint16 interrupter_target : 10;
  uint8 cycle : 1;
  uint8 toggle_cycle : 1;
  uint8 reserved1 : 2;
  uint8 chain : 1;
  uint8 interrupt_on_completion : 1;
  uint8 reserved2 : 4;
  uint8 type : 6;
  uint16 reserved3;
} ABSL_ATTRIBUTE_PACKED;
static_assert(sizeof(LinkTrb) == kTrbSize, "LinkTrb has incorrect size.");

struct EventDataTrb : public TrbBase {
  uint64 event_data;
  uint32 reserved0 : 22;
  uint16 interrupter_target : 10;
  uint8 cycle : 1;
  uint8 evaluate_next_trb : 1;
  uint8 reserved1 : 2;
  uint8 chain : 1;
  uint8 interrupt_on_completion : 1;
  uint8 reserved2 : 3;
  uint8 block_event_interrupt : 1;
  uint8 type : 6;
  uint16 reserved3 : 16;
} ABSL_ATTRIBUTE_PACKED;
static_assert(sizeof(EventDataTrb) == kTrbSize,
              "EventDataTrb has incorrect size.");

struct NoopTrb : public TrbBase {
  uint32 reserved0[2];
  uint32 reserved1 : 22;
  uint16 interrupter_target : 10;
  uint8 cycle : 1;
  uint8 evaluate_next_trb : 1;
  uint8 reserved2 : 2;
  uint8 chain : 1;
  uint8 interrupt_on_completion : 1;
  uint8 reserved3 : 4;
  uint8 type : 6;
  uint16 reserved4;
} ABSL_ATTRIBUTE_PACKED;
static_assert(sizeof(NoopTrb) == kTrbSize, "NoopTrb has incorrect size.");

struct DisableSlotCommandTrb : public TrbBase {
  uint32 reserved0[3];
  uint8 cycle : 1;
  uint16 reserved1 : 9;
  uint8 type : 6;
  uint8 reserved2;
  uint8 slot_id;
} ABSL_ATTRIBUTE_PACKED;
static_assert(sizeof(DisableSlotCommandTrb) == kTrbSize,
              "DisableSlotCommandTrb has incorrect size.");

struct AddressDeviceCommandTrb : public TrbBase {
  uint64 input_context_pointer;
  uint32 reserved;
  uint8 cycle : 1;
  uint8 reserved2 : 8;
  uint8 block_set_address_request : 1;
  uint8 type : 6;
  uint8 reserved3;
  uint8 slot_id;
} ABSL_ATTRIBUTE_PACKED;
static_assert(sizeof(AddressDeviceCommandTrb) == kTrbSize,
              "AddressDeviceCommandTrb has incorrect size.");

struct ConfigureEndpointCommandTrb : public TrbBase {
  uint64 input_context_pointer;
  uint32 reserved0;
  uint8 cycle : 1;
  uint8 reserved1 : 8;
  uint8 deconfigure : 1;
  uint8 type : 6;
  uint8 reserved2 : 8;
  uint8 slot_id;
} ABSL_ATTRIBUTE_PACKED;
static_assert(sizeof(ConfigureEndpointCommandTrb) == kTrbSize,
              "ConfigureEndpointCommandTrb has incorrect size.");

struct EvaluateContextCommandTrb : public TrbBase {
  uint64 input_context_pointer;
  uint32 reserved0;
  uint8 cycle : 1;
  uint16 reserved1 : 9;
  uint8 type : 6;
  uint8 reserved2;
  uint8 slot_id;
} ABSL_ATTRIBUTE_PACKED;
static_assert(sizeof(EvaluateContextCommandTrb) == kTrbSize,
              "EvaluateContextCommandTrb has incorrect size.");

struct ResetDeviceCommandTrb : public TrbBase {
  uint32 reserved0[3];
  uint8 cycle : 1;
  uint16 reserved1 : 9;
  uint8 type : 6;
  uint8 reserved2;
  uint8 slot_id;
} ABSL_ATTRIBUTE_PACKED;
static_assert(sizeof(ResetDeviceCommandTrb) == kTrbSize,
              "ResetDeviceCommandTrb has incorrect size.");

struct TransferEventTrb : public TrbBase {
  uint64 trb_pointer;
  uint32 trb_transfer_length : 24;
  uint8 completion_code;
  uint8 cycle : 1;
  uint8 reserved0 : 1;
  uint8 event_data : 1;
  uint8 reserved1 : 7;
  uint8 type : 6;
  uint8 endpoint_id : 5;
  uint8 reserved2 : 3;
  uint8 slot_id;
} ABSL_ATTRIBUTE_PACKED;

struct CommandCompletionEventTrb : public TrbBase {
  uint64 trb_pointer;
  uint32 command_completion_parameter : 24;
  TrbCompletionCode completion_code : 8;
  uint8 cycle : 1;
  uint16 reserved : 9;
  uint8 type : 6;
  uint8 vf_id;
  uint8 slot_id;
} ABSL_ATTRIBUTE_PACKED;

struct PortStatusChangeEventTrb : public TrbBase {
  uint32 reserved0 : 24;
  uint8 port_id;
  uint32 reserved1;
  uint32 reserved2 : 24;
  uint8 completion_code;
  uint8 cycle : 1;
  uint16 reserved3 : 9;
  uint8 type : 6;
  uint16 reserved4;
} ABSL_ATTRIBUTE_PACKED;

struct EventRingSegmentTableEntry {
  uint64 ring_segment_base_address;
  uint16 ring_segment_size;
  uint64 reserved2 : 48;

  string ToString() const {
    return absl::StrFormat("EventRingSegmentTableEntry: address=0x%X, size=%u",
                           ring_segment_base_address, ring_segment_size);
  }
} ABSL_ATTRIBUTE_PACKED;

struct InputControlContext {  // Xhci spec 6.2.5.1.
  uint32 drop_context_flags;
  uint32 add_context_flags;
  uint32 reserved1[5];
  uint8 configuration_value;
  uint8 interface_number;
  uint8 alternate_setting;
  uint8 reserved2;

  bool DropContextFlag(uint8 i) { return drop_context_flags & (1U << i); }

  bool AddContextFlag(uint8 i) { return add_context_flags & (1U << i); }
} ABSL_ATTRIBUTE_PACKED;

struct SlotContext {
  uint32 route_string : 20;
  uint8 speed : 4;
  uint8 reserved1 : 1;
  uint8 mtt : 1;
  uint8 hub : 1;
  uint8 context_entries : 5;
  uint16 max_exit_latency;
  uint8 root_hub_port_number;
  uint8 num_ports;
  uint8 tt_hub_slot_id;
  uint8 tt_port_number;
  uint8 tt_think_time : 2;
  uint8 reserved2 : 4;
  uint16 interrupter_target : 10;
  uint8 usb_device_address;
  uint32 reserved3 : 19;
  uint8 slot_state : 5;
  uint32 reserved4[4];
} ABSL_ATTRIBUTE_PACKED;

struct EndpointContext {
  uint8 endpoint_state : 3;
  uint8 reserved1 : 5;
  uint8 mult : 2;
  uint8 max_primary_streams : 5;
  uint8 linear_stream_array : 1;
  uint8 interval;
  uint8 max_esit_payload_hi;
  uint8 reserved2 : 1;
  uint8 error_count : 2;
  uint8 endpoint_type : 3;
  uint8 reserved3 : 1;
  uint8 host_initiate_disable : 1;
  uint8 max_burst_size : 8;
  uint16 max_packet_size;
  uint8 dequeue_cycle_state : 1;
  uint8 reserved4 : 3;
  uint64 tr_dequeue_pointer : 60;
  uint16 average_trb_length;
  uint16 max_esit_payload_lo;
  uint32 reserved5[3];
} ABSL_ATTRIBUTE_PACKED;

struct DeviceContext {
  SlotContext slot_context;
  EndpointContext endpoint_context[31];
} ABSL_ATTRIBUTE_PACKED;

// POD struct for associating a TRB with its address in guest memory.  This is
// useful because transfer and command completion event TRBs must contain
// pointers to the original TRB that generated the event.
struct AddressedTrb {
  Trb trb;
  GuestPhysicalAddress gpa;
};


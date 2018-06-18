// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

// An interrupter manages events on an event ring and signals an interrupt in
// the guest when necessary. Each interrupter is mapped to a unique MSI-X
// interrupt vector.
pub struct Interrupter {
    // Index of this interrupter.
    idx: u8,
    event_ring: EventRing,
    // True is the guest driver has enabled this interrupter.
    enabled: bool,
    pending: bool,
    event_handler_busy: bool,
    // TODO(jkwang) figure out moderation interval and counter
}

impl Interrupter {
  pub fn new() -> Interrupter {
  }

  pub fn add_event(&self, trb: Trb) {
      self.event_ring.add_event(trb);
      self.pending = true;
      self.maybe_signal_interrupt();
  }

  pub fn set_enabled(&self, enabled: bool) {
      self.enabled = enabled;
      self.maybe_signal_interrupt();
  }

  pub fn set_moderation(_interval: u16, _counter: u16) {
  }

  pub fn set_event_ring_seg_table_size(&self, size: u16) {
      self.event_ring.set_seg_table_size(size);
  }

  pub fn set_event_ring_seg_table_base_addr(&self, addr: GuestAddress) {
      self.event_ring.set_seg_table_base_addr(addr);
  }

  pub fn set_event_ring_dequeue_pointer(&self, addr: GuestAddress) {
      self.event_ring.set_dequeue_pointer(addr);
      // Refer to table 5-39.
      self.set_event_handler_busy(false);
      if self.event_ring.is_empty() {
          self.pending = false;
      }
  }

  pub fn set_event_handler_busy(&self, bool busy) {
      self.event_handler_busy = busy;
      self.maybe_signal_interrupt();
  }

  fn maybe_signal_interrupt(&self) {
  }
}

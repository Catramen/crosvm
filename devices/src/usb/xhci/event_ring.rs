// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

// Event rings are segmented circular buffers used to pass event TRBs from the
// xHCI device back to the guest.  Each event ring is associated with a single
// interrupter.  See section 4.9.4 of the xHCI specification for more details.
:

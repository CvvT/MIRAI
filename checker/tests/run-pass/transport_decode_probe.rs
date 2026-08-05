// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the root directory of this source tree.

// Faithful production-transport regression: a guarded lock precondition must survive a socket
// option whose discriminant is carried through a genuine `u32::from_ne_bytes` value-decode in a
// nested helper (not a literal) and then gated by a raw-integer `== 9` comparison across a
// dispatch-callback boundary. The precondition obligation forms only if MIRAI value-tracks the
// byte-decode transform through the comparison; before the decode-intrinsic fix this raw-integer
// gate was treated as opaque, dropping the KEEPALIVE warning (false negative). The aliased
// KEEPALIVE caller must warn; the same-shape BROADCAST caller must remain silent.

use mirai_annotations::*;

pub struct Lock;

fn require_unlocked_when(active: u32, lock: &Lock) {
    precondition!((active != 1) | (get_model_field!(lock, writer, 0usize) == 0));
    //~ related location
}

fn decode_socket_discriminant(raw: [u8; 4]) -> u32 {
    u32::from_ne_bytes(raw)
}

fn dispatch_transport_option(raw: [u8; 4], callback: impl FnOnce(u32)) {
    callback(decode_socket_discriminant(raw));
}

fn guarded_transport_decoded_option(raw: [u8; 4], read: &Lock, written: &Lock) {
    set_model_field!(written, writer, 1usize);
    dispatch_transport_option(raw, |selected| {
        if selected == 9 {
            require_unlocked_when(1, read);
        }
    });
}

pub fn transport_decoded_keepalive_alias(lock: &Lock) {
    set_model_field!(lock, writer, 0usize);
    guarded_transport_decoded_option(9u32.to_ne_bytes(), lock, lock);
    //~ possible alias violates precondition
}

pub fn transport_decoded_broadcast_alias(lock: &Lock) {
    set_model_field!(lock, writer, 0usize);
    guarded_transport_decoded_option(6u32.to_ne_bytes(), lock, lock);
}

pub fn main() {}

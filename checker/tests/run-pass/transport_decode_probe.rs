// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the root directory of this source tree.

// Corroborating production-transport coverage: a socket discriminant flows through a genuine
// `u32::from_ne_bytes` value-decode in a nested helper and a raw-integer `== 9` gate across a
// dispatch-callback boundary. Because the guarded call passes a literal `1`, its precondition is
// decode-independent; this fixture is not a reliable pre-fix differential. The aliased KEEPALIVE
// caller must warn, while the same-shape BROADCAST caller must remain silent.

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

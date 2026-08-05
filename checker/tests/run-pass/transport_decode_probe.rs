// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the root directory of this source tree.

// Faithful production-transport regression: a guarded lock precondition must survive the
// real nested-decode socket-option shape -- grouped tuple match, two callback arguments, a
// nested writer callback, and the option discriminant carried through a genuine
// `u32::from_ne_bytes` value-decode (not a literal). The aliased KEEPALIVE caller must warn;
// the same-shape BROADCAST caller must remain silent.

use mirai_annotations::*;

pub struct Lock;

fn require_unlocked_when(active: u32, lock: &Lock) {
    precondition!((active != 1) | (get_model_field!(lock, writer, 0usize) == 0));
    //~ related location
}

#[derive(Clone, Copy)]
enum SocketOption {
    Broadcast,
    KeepAlive,
}

enum SocketOptionValue {
    U32(u32),
}

fn with_writer(written: &Lock, callback: impl FnOnce()) {
    set_model_field!(written, writer, 1usize);
    callback();
}

fn transport_decode_socket_option(
    raw: [u8; 4],
    callback: impl FnOnce(SocketOption, SocketOptionValue),
) {
    let discriminant = u32::from_ne_bytes(raw);
    let selected = if discriminant == 9 {
        SocketOption::KeepAlive
    } else {
        SocketOption::Broadcast
    };
    callback(selected, SocketOptionValue::U32(discriminant));
}

fn guarded_transport_decoded_option(raw: [u8; 4], read: &Lock, written: &Lock) {
    transport_decode_socket_option(raw, |selected, value| {
        with_writer(written, || match (selected, value) {
            (SocketOption::KeepAlive, SocketOptionValue::U32(value)) => {
                let _enabled = value != 0;
                require_unlocked_when(1, read)
            }
            (SocketOption::Broadcast, SocketOptionValue::U32(value)) => {
                let _enabled = value != 0;
            }
        });
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

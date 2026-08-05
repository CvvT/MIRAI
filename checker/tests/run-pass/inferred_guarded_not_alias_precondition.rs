// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the root directory of this source tree.

// A guarded lock precondition must infer a guarded non-alias obligation. The aliased call is invalid
// only while `active == 1`; inactive and distinct-lock callers must remain accepted.

use mirai_annotations::*;

pub struct Lock;

fn require_unlocked_when(active: u32, lock: &Lock) {
    precondition!((active != 1) | (get_model_field!(lock, writer, 0usize) == 0));
    //~ related location
    //~ related location
    //~ related location
}

fn guarded_distinct_writers(active: u32, read: &Lock, written: &Lock) {
    set_model_field!(written, writer, 1usize);
    require_unlocked_when(active, read); //~ related location
}

fn invoke_when_active(active: u32, callback: impl FnOnce()) {
    if active == 1 {
        callback();
    }
}

fn guarded_callback_writers(active: u32, read: &Lock, written: &Lock) {
    set_model_field!(written, writer, 1usize);
    invoke_when_active(active, || require_unlocked_when(1, read)); //~ related location
    //~ related location
}

#[derive(Clone, Copy)]
enum SocketOption {
    Broadcast,
    KeepAlive,
}

enum SocketOptionName {
    Socket(SocketOption),
}

enum SocketOptionValue {
    U32(u32),
}

fn with_writer(written: &Lock, callback: impl FnOnce()) {
    set_model_field!(written, writer, 1usize);
    callback();
}

fn guarded_socket_option(option: SocketOption, read: &Lock, written: &Lock) {
    with_writer(written, || match option { //~ related location
        SocketOption::KeepAlive => require_unlocked_when(1, read),
        SocketOption::Broadcast => {}
    });
}

fn dispatch_socket_option(option: SocketOption, callback: impl FnOnce(SocketOption)) {
    match option {
        SocketOption::Broadcast => callback(SocketOption::Broadcast),
        SocketOption::KeepAlive => callback(SocketOption::KeepAlive),
    }
}

fn guarded_dispatched_socket_option(option: SocketOption, read: &Lock, written: &Lock) {
    set_model_field!(written, writer, 1usize);
    dispatch_socket_option(option, |selected| match selected { //~ related location
        SocketOption::KeepAlive => require_unlocked_when(1, read),
        SocketOption::Broadcast => {}
    });
}

fn dispatch_named_socket_option(
    option: SocketOptionName,
    callback: impl FnOnce(SocketOption),
) {
    match option {
        SocketOptionName::Socket(selected) => callback(selected),
    }
}

fn guarded_named_socket_option(option: SocketOptionName, read: &Lock, written: &Lock) {
    set_model_field!(written, writer, 1usize);
    dispatch_named_socket_option(option, |selected| match selected {
        SocketOption::KeepAlive => require_unlocked_when(1, read),
        SocketOption::Broadcast => {}
    });
}

fn decode_socket_option(
    option: SocketOptionName,
    callback: impl FnOnce(SocketOption, SocketOptionValue),
) {
    match option {
        SocketOptionName::Socket(selected @ (SocketOption::Broadcast | SocketOption::KeepAlive)) => {
            callback(selected, SocketOptionValue::U32(1))
        }
    }
}

fn guarded_decoded_socket_option(option: SocketOptionName, read: &Lock, written: &Lock) {
    decode_socket_option(option, |selected, value| {
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

fn dispatch_byte_decoded_option(option: [u8; 4], callback: impl FnOnce(u32)) {
    callback(u32::from_ne_bytes(option));
}

fn guarded_byte_decoded_option(option: [u8; 4], read: &Lock, written: &Lock) {
    set_model_field!(written, writer, 1usize);
    dispatch_byte_decoded_option(option, |selected| {
        if selected == 9 {
            require_unlocked_when(1, read);
        }
    }); //~ related location
}

pub fn active_alias(lock: &Lock) {
    set_model_field!(lock, writer, 0usize);
    guarded_distinct_writers(1, lock, lock); //~ possible alias violates precondition
}

pub fn inactive_alias(lock: &Lock) {
    set_model_field!(lock, writer, 0usize);
    guarded_distinct_writers(0, lock, lock);
}

pub fn active_distinct(read: &Lock, written: &Lock) {
    assume!(not_alias!(read, written));
    set_model_field!(read, writer, 0usize);
    guarded_distinct_writers(1, read, written);
}

pub fn active_callback_alias(lock: &Lock) {
    set_model_field!(lock, writer, 0usize);
    guarded_callback_writers(1, lock, lock); //~ possible alias violates precondition
}

pub fn inactive_callback_alias(lock: &Lock) {
    set_model_field!(lock, writer, 0usize);
    guarded_callback_writers(0, lock, lock);
}

pub fn keepalive_alias(lock: &Lock) {
    set_model_field!(lock, writer, 0usize);
    guarded_socket_option(SocketOption::KeepAlive, lock, lock);
    //~ possible alias violates precondition
}

pub fn broadcast_alias(lock: &Lock) {
    set_model_field!(lock, writer, 0usize);
    guarded_socket_option(SocketOption::Broadcast, lock, lock);
}

pub fn dispatched_keepalive_alias(lock: &Lock) {
    set_model_field!(lock, writer, 0usize);
    guarded_dispatched_socket_option(SocketOption::KeepAlive, lock, lock);
    //~ possible alias violates precondition
}

pub fn dispatched_broadcast_alias(lock: &Lock) {
    set_model_field!(lock, writer, 0usize);
    guarded_dispatched_socket_option(SocketOption::Broadcast, lock, lock);
}

pub fn named_keepalive_alias(lock: &Lock) {
    set_model_field!(lock, writer, 0usize);
    guarded_named_socket_option(
        SocketOptionName::Socket(SocketOption::KeepAlive),
        lock,
        lock,
    );
    //~ possible alias violates precondition
}

pub fn named_broadcast_alias(lock: &Lock) {
    set_model_field!(lock, writer, 0usize);
    guarded_named_socket_option(
        SocketOptionName::Socket(SocketOption::Broadcast),
        lock,
        lock,
    );
}

pub fn decoded_keepalive_alias(lock: &Lock) {
    set_model_field!(lock, writer, 0usize);
    guarded_decoded_socket_option(
        SocketOptionName::Socket(SocketOption::KeepAlive),
        lock,
        lock,
    );
    //~ possible alias violates precondition
}

pub fn decoded_broadcast_alias(lock: &Lock) {
    set_model_field!(lock, writer, 0usize);
    guarded_decoded_socket_option(
        SocketOptionName::Socket(SocketOption::Broadcast),
        lock,
        lock,
    );
}

pub fn byte_decoded_keepalive_alias(lock: &Lock) {
    set_model_field!(lock, writer, 0usize);
    guarded_byte_decoded_option(9u32.to_ne_bytes(), lock, lock);
    //~ possible alias violates precondition
}

pub fn byte_decoded_broadcast_alias(lock: &Lock) {
    set_model_field!(lock, writer, 0usize);
    guarded_byte_decoded_option(6u32.to_ne_bytes(), lock, lock);
}

fn dispatch_shift_decoded_option(option: [u8; 4], callback: impl FnOnce(u32)) {
    let selected = (option[0] as u32)
        | ((option[1] as u32) << 8)
        | ((option[2] as u32) << 16)
        | ((option[3] as u32) << 24);
    callback(selected);
}

fn guarded_shift_decoded_option(option: [u8; 4], read: &Lock, written: &Lock) {
    set_model_field!(written, writer, 1usize);
    dispatch_shift_decoded_option(option, |selected| {
        if selected == 9 {
            require_unlocked_when(1, read);
        }
    });
}

pub fn shift_decoded_keepalive_alias(lock: &Lock) {
    set_model_field!(lock, writer, 0usize);
    guarded_shift_decoded_option([9, 0, 0, 0], lock, lock);
    //~ possible alias violates precondition
}

pub fn shift_decoded_broadcast_alias(lock: &Lock) {
    set_model_field!(lock, writer, 0usize);
    guarded_shift_decoded_option([6, 0, 0, 0], lock, lock);
}

// Admissibility control for the byte-decode value-tracking above: decode a u32 through
// `from_ne_bytes` and then require *unconditionally* (no `== 9` discriminant guard). The alias
// obligation must still form and be reported, proving `from_ne_bytes` yields a value MIRAI can
// track to the precondition -- i.e. the guarded decoded false negative that existed before the
// decode-intrinsic fix was an admissible discriminant-precision defect, not an opaque modeling
// gap. This cell warns independently of discriminant recovery and guards against regressing the
// value-tracking of the byte transform itself.
fn unconditional_byte_decoded_option(option: [u8; 4], read: &Lock, written: &Lock) {
    set_model_field!(written, writer, 1usize);
    dispatch_byte_decoded_option(option, |_selected| {
        require_unlocked_when(1, read);
    }); //~ related location
}

pub fn unconditional_byte_decoded_alias(lock: &Lock) {
    set_model_field!(lock, writer, 0usize);
    unconditional_byte_decoded_option(9u32.to_ne_bytes(), lock, lock);
    //~ possible alias violates precondition
}

pub fn main() {}

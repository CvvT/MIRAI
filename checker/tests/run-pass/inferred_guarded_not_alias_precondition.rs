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
    dispatch_socket_option(option, |selected| match selected {
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

pub fn main() {}

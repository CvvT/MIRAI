#![allow(unexpected_cfgs)]

use mirai_annotations::precondition;

struct State {
    ready: bool,
}

fn require_ready(state: &State) {
    precondition!(state.ready, "callback requires prepared state");
}

fn prepare_then_invoke<F>(state: &mut State, callback: F)
where
    F: FnOnce(&State),
{
    state.ready = true;
    callback(state);
}

fn main() {
    let mut state = State { ready: false };
    prepare_then_invoke(&mut state, require_ready);
}

fn main() {
    let select_first = std::hint::black_box(true);
    let callback = || {};
    let first = &callback;
    let second = &callback;
    let ambiguous = move || {
        let selected = if select_first { first } else { second };
        selected();
    };
    ambiguous();
}

use std::sync::atomic::{AtomicBool, Ordering};

static FN_KEY_PRESSED: AtomicBool = AtomicBool::new(false);

pub fn is_fn_pressed() -> bool {
    FN_KEY_PRESSED.load(Ordering::SeqCst)
}

pub fn set_fn_pressed(pressed: bool) {
    FN_KEY_PRESSED.store(pressed, Ordering::SeqCst);
}

pub fn toggle_fn_pressed() -> bool {
    let current = FN_KEY_PRESSED.load(Ordering::SeqCst);
    FN_KEY_PRESSED.store(!current, Ordering::SeqCst);
    !current
}

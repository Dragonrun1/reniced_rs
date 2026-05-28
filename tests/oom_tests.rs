use reniced::actions::convert_oom_adj;

#[test]
fn converts_maximum_oom_adj() {
    // The legacy maximum maps to the new-interface maximum.
    assert_eq!(convert_oom_adj(15), 1000);
}

#[test]
fn converts_zero_oom_adj() {
    assert_eq!(convert_oom_adj(0), 0);
}

#[test]
fn converts_negative_oom_adj() {
    assert_eq!(convert_oom_adj(-17), -1000);
}

#[test]
fn converts_mid_range_positive() {
    // Formula: (score * 1000) / 17 => (7 * 1000) / 17 = 411 (integer division)
    assert_eq!(convert_oom_adj(7), 411);
}

#[test]
fn converts_mid_range_negative() {
    // (-8 * 1000) / 17 = -470 (integer division toward zero in Rust)
    assert_eq!(convert_oom_adj(-8), -470);
}

#[test]
fn converts_one() {
    assert_eq!(convert_oom_adj(1), 58);
}

#[test]
fn converts_minus_one() {
    assert_eq!(convert_oom_adj(-1), -58);
}

use reniced::actions::convert_oom_adj;

#[test]
fn converts_maximum_oom_adj() {
    assert_eq!(convert_oom_adj(15), 1000);
}

#[test]
fn converts_negative_oom_adj() {
    assert_eq!(convert_oom_adj(-17), -1000);
}

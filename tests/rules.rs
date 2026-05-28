use anyhow::Result;
use reniced::config::parse_rule;

#[test]
fn parses_combined_rule() -> Result<()> {
    let rule = parse_rule("n-10r4o5", "^seti")?;

    assert_eq!(rule.nice, Some(-10));
    assert_eq!(rule.oom_adj, Some(5));

    Ok(())
}

#[test]
fn rejects_rule_with_no_actions() {
    let result = parse_rule("xyz", "someprocess");
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("no recognised actions"));
}

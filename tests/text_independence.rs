#![allow(dead_code)]

#[path = "../src/text/mod.rs"]
pub mod text;

#[test]
fn report_uses_only_the_supplied_language_and_limit() {
    use text::{Language, report::Report};

    let mut english = Report::new(Language::English, 2_000);
    english.succeeded.push("first".into());
    assert!(english.render().contains("[Completed]"));

    let mut chinese = Report::new(Language::ChineseSimplified, 80);
    chinese.succeeded = vec!["a".repeat(50), "b".repeat(50), "c".repeat(50)];
    let rendered = chinese.render();
    assert!(rendered.contains("[完成明细]"));
    assert!(rendered.contains("省略"));
    assert!(rendered.encode_utf16().count() <= 80);
}

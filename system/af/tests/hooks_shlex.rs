//! The guards' tokenizer against CPython 3.12's `shlex.split(s, posix=True)`: the cases in
//! hooks_shlex_cases.json were produced by that function (null marks its ValueError).

use afcore::hooks::shlex::split_posix;
use serde_json::Value;

#[test]
fn test_split_posix_matches_cpython() {
    let text = include_str!("hooks_shlex_cases.json");
    let cases: Vec<(String, Option<Vec<String>>)> = serde_json::from_str::<Vec<Value>>(text)
        .unwrap()
        .into_iter()
        .map(|c| (c[0].as_str().unwrap().to_string(), serde_json::from_value(c[1].clone()).unwrap()))
        .collect();
    assert!(cases.len() > 200);
    for (input, want) in cases {
        assert_eq!(split_posix(&input).ok(), want, "input {input:?}");
    }
}

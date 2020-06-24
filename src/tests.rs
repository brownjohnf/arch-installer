use super::*;

#[test]
fn test_exec() {
    std::str::from_utf8(&exec(&[&"echo", &"true"]).unwrap().stdout).unwrap();
    std::str::from_utf8(&exec(&[&"echo"]).unwrap().stdout).unwrap();
    std::str::from_utf8(&exec(&[]).unwrap().stdout).unwrap();
    assert_eq!(
        std::str::from_utf8(&exec(&[&"echo", &"true"]).unwrap().stdout)
            .unwrap()
            .trim(),
        "true"
    );
}

#[test]
fn test_select() {
    assert_eq!(
        select(
            "please select item 2",
            &["item 1".to_string(), "item 2".to_string()]
        )
        .unwrap(),
        1,
    );
}

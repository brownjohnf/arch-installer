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

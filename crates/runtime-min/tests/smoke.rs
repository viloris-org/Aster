use runtime_min::smoke_runtime_min;

#[test]
fn native_runtime_min_smoke_test() {
    assert_eq!(smoke_runtime_min().unwrap(), 1);
}

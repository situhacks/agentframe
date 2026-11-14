//! `af` runs its verbs on a thread with a large stack (`cli::run_on_stack`); the process must still end
//! the way it did on the main thread.

use afcore::cli::run_on_stack;

#[test]
fn the_exit_code_and_the_argv_come_back() {
    let code = run_on_stack(16 * 1024 * 1024, vec!["a".into(), "b".into()], |argv| if argv == ["a", "b"] { 7 } else { 1 });
    assert_eq!(code, 7);
}

#[test]
fn a_panic_in_the_worker_is_exit_101_as_on_the_main_thread() {
    assert_eq!(run_on_stack(16 * 1024 * 1024, vec![], |_| panic!("boom")), 101);
}

#[test]
fn the_worker_has_the_stack_it_was_given() {
    // 8 MB of locals would overflow a 1 MB main thread; the worker's 32 MB holds them.
    let code = run_on_stack(32 * 1024 * 1024, vec![], |_| {
        let big = std::hint::black_box([1u8; 8 * 1024 * 1024]);
        (big[0] + big[big.len() - 1]) as i32
    });
    assert_eq!(code, 2);
}

#[test]
fn a_thread_that_cannot_start_is_a_non_zero_exit() {
    assert_eq!(run_on_stack(usize::MAX / 2, vec![], |_| 0), 1);
}

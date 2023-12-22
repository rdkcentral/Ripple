#[cfg(test)]
pub mod tests {
    use ripple_proc_macros::counted;
    use ripple_sdk::tokio;
    use std::{thread, time::Duration};

    #[counted]
    pub fn stand_up_and_be_counted_no_args() {
        println!("asdfasdf");
    }

    #[counted]
    pub fn stand_up_and_be_counted_with_args(_input: String, _count: u32) {
        println!("asdfasdf");
        thread::sleep(Duration::new(1, 0));
    }
    #[counted]
    pub async fn async_stand_up_and_be_counted_no_args() {
        println!("asdfasdf,now with async");
        thread::sleep(Duration::new(1, 0));
    }
    #[test]
    pub fn test_counted_no_args() {
        stand_up_and_be_counted_no_args();
        assert!(true);
    }

    #[test]
    pub fn test_counted_with_args() {
        stand_up_and_be_counted_with_args(String::from("foo"), 42);
        assert!(true);
    }
    #[tokio::test]
    pub async fn async_test_counted_no_args() {
        async_stand_up_and_be_counted_no_args().await;
        assert!(true);
    }
}

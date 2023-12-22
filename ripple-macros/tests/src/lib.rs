// #[macro_use]
// extern crate ripple_proc_macros;
// extern crate ripple_sdk;
//extern crate tokio;

use ripple_sdk::tokio;
use ripple_proc_macros::counted;


#[cfg(test)]
pub mod tests {
    use std::thread;
    use ripple_sdk::tokio;
    use ripple_proc_macros::counted;

    #[counted]
    pub fn stand_up_and_be_counted_no_args() {
       println!("asdfasdf");
    }

    #[counted]
    pub fn stand_up_and_be_counted_with_args(input: String,count: u32) {
       println!("asdfasdf");
       thread::sleep_ms(1000);
    }
    #[counted]
    pub async fn async_stand_up_and_be_counted_no_args() {
       println!("asdfasdf,now with async");
       thread::sleep_ms(1000);
    }
    #[test]
    pub fn test_counted_no_args() {
        stand_up_and_be_counted_no_args();
        assert!(true);
    }

    #[test]
    pub fn test_counted_with_args() {
        stand_up_and_be_counted_with_args(String::from("foo"),42);
        assert!(true);
    }
    #[tokio::test]
    pub async fn async_test_counted_no_args() {
        async_stand_up_and_be_counted_no_args().await;
        assert!(true);
    }

}
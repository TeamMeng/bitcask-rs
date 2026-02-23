#![allow(unused)]

use bytes::Bytes;

pub fn get_test_key(i: i32) -> Bytes {
    Bytes::from(format!("bitcasl-rs-key-{:09}", i))
}

pub fn get_test_value(i: i32) -> Bytes {
    Bytes::from(format!("bitcasl-rs-value-{:09}", i))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_test_key_and_value_should_work() {
        for i in 0..10 {
            assert_eq!(get_test_key(i), format!("bitcasl-rs-key-{:09}", i));
            assert_eq!(get_test_value(i), format!("bitcasl-rs-value-{:09}", i));
        }
    }
}

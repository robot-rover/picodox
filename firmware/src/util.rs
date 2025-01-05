use embassy_sync::blocking_mutex::raw::NoopRawMutex;

pub type MutexType = NoopRawMutex;

macro_rules! async_unwrap {
    (op $option:expr, $($error_args:expr),*) => {{
        match $option {
            Some(value) => value,
            None => async_panic!($($error_args),*),
        }
    }};
    (res $result:expr, $($error_args:expr),*) => {{
        match $result {
            Ok(value) => value,
            Err(err) => async_panic!($($error_args),*, err),
        }
    }};
}

macro_rules! async_panic {
    ($($error_args:expr),*) => {{
        use core::future::pending;
        use defmt::error;
        error!($($error_args),*);
        pending().await
    }};
}

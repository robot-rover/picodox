macro_rules! async_unwrap {
    (op $option:expr, $($error_args:tt),*) => {{
        match $option {
            Some(value) => value,
            None => async_panic!($($error_args)*,),
        }
    }};
    (res $result:expr, $($error_args:tt),*) => {{
        match $result {
            Ok(value) => value,
            Err(err) => async_panic!($($error_args),*, err),
        }
    }};
}

macro_rules! async_panic {
    ($($error_args:tt),*) => {{
        use core::future::pending;
        use defmt::error;
        error!($($error_args),*);
        pending().await
    }};
}

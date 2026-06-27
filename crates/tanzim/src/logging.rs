#[doc(hidden)]
#[macro_export]
macro_rules! is_debug_level_enabled {
    () => {{
        cfg_if::cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::enabled!(tracing::Level::DEBUG)
            } else if #[cfg(feature = "logging")] {
                log::log_enabled!(log::Level::Debug)
            } else {
                false
            }
        }
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! is_trace_level_enabled {
    () => {{
        cfg_if::cfg_if! {
            if #[cfg(feature = "tracing")] {
                tracing::enabled!(tracing::Level::TRACE)
            } else if #[cfg(feature = "logging")] {
                log::log_enabled!(log::Level::Trace)
            } else {
                false
            }
        }
    }};
}

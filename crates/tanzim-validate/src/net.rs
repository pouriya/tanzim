use crate::error::{Error, ErrorKind};
use crate::{Meta, Validator};
use tanzim_value::{Value, ValueType};

/// RFC 1123 hostname check: 1–253 chars, dot-separated labels of 1–63 chars made of
/// ASCII letters, digits, and hyphens, with no leading or trailing hyphen per label.
fn is_hostname(host: &str) -> bool {
    if host.is_empty() || host.len() > 253 {
        return false;
    }
    for label in host.split('.') {
        let bytes = label.as_bytes();
        if bytes.is_empty() || bytes.len() > 63 {
            return false;
        }
        if bytes[0] == b'-' || bytes[bytes.len() - 1] == b'-' {
            return false;
        }
        for &byte in bytes {
            if !byte.is_ascii_alphanumeric() && byte != b'-' {
                return false;
            }
        }
    }
    true
}

/// Borrow the inner string, or produce a `Type` error expecting a string.
fn as_string(value: &mut Value) -> Result<&mut String, Error> {
    match value {
        Value::String(text) => Ok(text),
        other => Err(Error::new(ErrorKind::Type {
            expected: ValueType::String,
            found: other.type_name(),
        })),
    }
}

/// (`net` feature) Accepts a hostname or an IP address literal.
#[derive(Debug, Clone, Default)]
pub struct Host {
    meta: Meta,
}

impl Host {
    /// A new, unconfigured `Host` validator.
    pub fn new() -> Self {
        Self {
            meta: Meta::default(),
        }
    }

    /// Attach human-facing metadata (name, description, examples, default, output conversion).
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = meta;
        self
    }
}

crate::impl_meta_methods!(Host);

impl Validator for Host {
    fn meta(&self) -> &Meta {
        &self.meta
    }

    fn meta_mut(&mut self) -> &mut Meta {
        &mut self.meta
    }

    fn check(&self, value: &mut Value) -> Result<(), Error> {
        let text = as_string(value)?;
        if text.parse::<std::net::IpAddr>().is_ok() || is_hostname(text) {
            Ok(())
        } else {
            Err(Error::new(ErrorKind::Format { expected: "host" }))
        }
    }
}

/// (`net` feature) Accepts a DNS domain name, normalizing it to lowercase.
#[derive(Debug, Clone, Default)]
pub struct Domain {
    meta: Meta,
    require_dot: bool,
}

impl Domain {
    /// Attach human-facing metadata (name, description, examples, default, output conversion).
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = meta;
        self
    }

    /// A new, unconfigured `Domain` validator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Require at least one dot (reject bare labels like `localhost`).
    pub fn require_dot(mut self) -> Self {
        self.require_dot = true;
        self
    }
}

crate::impl_meta_methods!(Domain);

impl Validator for Domain {
    fn meta(&self) -> &Meta {
        &self.meta
    }

    fn meta_mut(&mut self) -> &mut Meta {
        &mut self.meta
    }

    fn check(&self, value: &mut Value) -> Result<(), Error> {
        let text = as_string(value)?;
        *text = text.to_lowercase();
        if !is_hostname(text) || (self.require_dot && !text.contains('.')) {
            return Err(Error::new(ErrorKind::Format { expected: "domain" }));
        }
        Ok(())
    }
}

/// (`net` feature) Accepts an email address, normalizing the domain part to lowercase.
#[derive(Debug, Clone, Default)]
pub struct Email {
    meta: Meta,
}

impl Email {
    /// A new, unconfigured `Email` validator.
    pub fn new() -> Self {
        Self {
            meta: Meta::default(),
        }
    }

    /// Attach human-facing metadata (name, description, examples, default, output conversion).
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = meta;
        self
    }
}

crate::impl_meta_methods!(Email);

impl Validator for Email {
    fn meta(&self) -> &Meta {
        &self.meta
    }

    fn meta_mut(&mut self) -> &mut Meta {
        &mut self.meta
    }

    fn check(&self, value: &mut Value) -> Result<(), Error> {
        let text = as_string(value)?;
        let (local, domain) = match text.rsplit_once('@') {
            Some(parts) => parts,
            None => return Err(Error::new(ErrorKind::Format { expected: "email" })),
        };
        if local.is_empty() || local.len() > 64 || !is_hostname(domain) || !domain.contains('.') {
            return Err(Error::new(ErrorKind::Format { expected: "email" }));
        }
        *text = format!("{local}@{}", domain.to_lowercase());
        Ok(())
    }
}

/// (`net` feature) Accepts a TCP/UDP port number, coercing numeric strings and floats like [`crate::Integer`].
#[derive(Debug, Clone)]
pub struct Port {
    meta: Meta,
    allow_zero: bool,
    privileged_ok: bool,
}

impl Default for Port {
    fn default() -> Self {
        Self {
            meta: Meta::default(),
            allow_zero: false,
            privileged_ok: true,
        }
    }
}

impl Port {
    /// Attach human-facing metadata (name, description, examples, default, output conversion).
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = meta;
        self
    }

    /// A new `Port` validator (privileged ports allowed, `0` rejected by default).
    pub fn new() -> Self {
        Self::default()
    }

    /// Permit port `0` (e.g. "pick any free port").
    pub fn allow_zero(mut self) -> Self {
        self.allow_zero = true;
        self
    }

    /// When `false`, reject privileged ports below 1024.
    pub fn privileged_ok(mut self, allowed: bool) -> Self {
        self.privileged_ok = allowed;
        self
    }
}

crate::impl_meta_methods!(Port);

impl Validator for Port {
    fn meta(&self) -> &Meta {
        &self.meta
    }

    fn meta_mut(&mut self) -> &mut Meta {
        &mut self.meta
    }

    fn check(&self, value: &mut Value) -> Result<(), Error> {
        let min = if self.allow_zero { 0 } else { 1 };
        crate::Integer::new().range(min, 65535).validate(value)?;
        let port = match value.as_int() {
            Some(port) => port,
            None => unreachable!("Integer validation produced a non-integer"),
        };
        if !self.privileged_ok && (1..1024).contains(&port) {
            return Err(Error::new(ErrorKind::Format {
                expected: "non-privileged port (>= 1024)",
            }));
        }
        Ok(())
    }
}

/// (`net` feature) Accepts an IP address literal.
#[derive(Debug, Clone, Default)]
pub struct IpAddr {
    meta: Meta,
    v4_only: bool,
    v6_only: bool,
}

impl IpAddr {
    /// Attach human-facing metadata (name, description, examples, default, output conversion).
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = meta;
        self
    }

    /// A new `IpAddr` validator accepting both IPv4 and IPv6.
    pub fn new() -> Self {
        Self::default()
    }

    /// Restrict to IPv4 addresses only.
    pub fn v4_only(mut self) -> Self {
        self.v4_only = true;
        self.v6_only = false;
        self
    }

    /// Restrict to IPv6 addresses only.
    pub fn v6_only(mut self) -> Self {
        self.v6_only = true;
        self.v4_only = false;
        self
    }
}

crate::impl_meta_methods!(IpAddr);

impl Validator for IpAddr {
    fn meta(&self) -> &Meta {
        &self.meta
    }

    fn meta_mut(&mut self) -> &mut Meta {
        &mut self.meta
    }

    fn check(&self, value: &mut Value) -> Result<(), Error> {
        let text = as_string(value)?;
        let parsed = match text.parse::<std::net::IpAddr>() {
            Ok(parsed) => parsed,
            Err(_) => {
                return Err(Error::new(ErrorKind::Format {
                    expected: "ip address",
                }));
            }
        };
        if self.v4_only && !parsed.is_ipv4() {
            return Err(Error::new(ErrorKind::Format {
                expected: "IPv4 address",
            }));
        }
        if self.v6_only && !parsed.is_ipv6() {
            return Err(Error::new(ErrorKind::Format {
                expected: "IPv6 address",
            }));
        }
        Ok(())
    }
}

/// (`net` feature) Accepts a `host:port` socket address (IP or hostname host).
#[derive(Debug, Clone, Default)]
pub struct SocketAddr {
    meta: Meta,
}

impl SocketAddr {
    /// A new, unconfigured `SocketAddr` validator.
    pub fn new() -> Self {
        Self {
            meta: Meta::default(),
        }
    }

    /// Attach human-facing metadata (name, description, examples, default, output conversion).
    pub fn with_meta(mut self, meta: Meta) -> Self {
        self.meta = meta;
        self
    }
}

crate::impl_meta_methods!(SocketAddr);

impl Validator for SocketAddr {
    fn meta(&self) -> &Meta {
        &self.meta
    }

    fn meta_mut(&mut self) -> &mut Meta {
        &mut self.meta
    }

    fn check(&self, value: &mut Value) -> Result<(), Error> {
        let text = as_string(value)?;
        if text.parse::<std::net::SocketAddr>().is_ok() {
            return Ok(());
        }
        // hostname:port form (std only parses ip:port)
        if let Some((host, port)) = text.rsplit_once(':') {
            let port_ok = match port.parse::<u16>() {
                Ok(number) => number != 0,
                Err(_) => false,
            };
            if port_ok && is_hostname(host) {
                return Ok(());
            }
        }
        Err(Error::new(ErrorKind::Format {
            expected: "socket address",
        }))
    }
}

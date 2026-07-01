use crate::Validator;
use crate::error::{Error, ErrorKind};
use tanzim_value::{Value, ValueType};

/// (`cidr` feature) Accepts a CIDR network such as `10.0.0.0/8` or `2001:db8::/32`.
#[derive(Debug, Clone, Default)]
pub struct Cidr;

impl Cidr {
    pub fn new() -> Self {
        Self
    }
}

impl Validator for Cidr {
    fn validate(&self, value: &mut Value) -> Result<(), Error> {
        let text = match value {
            Value::String(text) => text,
            other => {
                return Err(Error::new(ErrorKind::Type {
                    expected: ValueType::String,
                    found: other.type_name(),
                }));
            }
        };
        match text.parse::<ipnet::IpNet>() {
            Ok(_) => Ok(()),
            Err(_) => Err(Error::new(ErrorKind::Format {
                expected: "CIDR network",
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_and_rejects() {
        assert!(
            Cidr::new()
                .validate(&mut Value::String("10.0.0.0/8".into()))
                .is_ok()
        );
        assert!(
            Cidr::new()
                .validate(&mut Value::String("10.0.0.0".into()))
                .is_err()
        );
    }
}

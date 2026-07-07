use tanzim_validate::{Domain, Email, Host, IpAddr, Port, SocketAddr, Validator};
use tanzim_value::Value;

fn string(text: &str) -> Value {
    Value::String(text.to_string())
}

#[test]
fn host_accepts_name_and_ip() {
    assert!(Host::new().validate(&mut string("example.com")).is_ok());
    assert!(Host::new().validate(&mut string("127.0.0.1")).is_ok());
    assert!(Host::new().validate(&mut string("bad_host!")).is_err());
}

#[test]
fn domain_lowercases_and_requires_dot() {
    let mut value = string("Example.COM");
    Domain::new().require_dot().validate(&mut value).unwrap();
    assert_eq!(value, string("example.com"));
    assert!(
        Domain::new()
            .require_dot()
            .validate(&mut string("localhost"))
            .is_err()
    );
}

#[test]
fn email_validates_and_lowercases_domain() {
    let mut value = string("User@Example.COM");
    Email::new().validate(&mut value).unwrap();
    assert_eq!(value, string("User@example.com"));
    assert!(Email::new().validate(&mut string("nope")).is_err());
}

#[test]
fn port_range_and_privileged() {
    let mut value = string("8080");
    Port::new().validate(&mut value).unwrap();
    assert_eq!(value, Value::Int(8080));
    assert!(Port::new().validate(&mut Value::Int(0)).is_err());
    assert!(
        Port::new()
            .allow_zero()
            .validate(&mut Value::Int(0))
            .is_ok()
    );
    assert!(
        Port::new()
            .privileged_ok(false)
            .validate(&mut Value::Int(80))
            .is_err()
    );
}

#[test]
fn ip_addr_family_filter() {
    assert!(
        IpAddr::new()
            .v4_only()
            .validate(&mut string("10.0.0.1"))
            .is_ok()
    );
    assert!(
        IpAddr::new()
            .v4_only()
            .validate(&mut string("::1"))
            .is_err()
    );
    assert!(IpAddr::new().v6_only().validate(&mut string("::1")).is_ok());
}

#[test]
fn socket_addr_forms() {
    assert!(
        SocketAddr::new()
            .validate(&mut string("127.0.0.1:8080"))
            .is_ok()
    );
    assert!(
        SocketAddr::new()
            .validate(&mut string("example.com:443"))
            .is_ok()
    );
    assert!(
        SocketAddr::new()
            .validate(&mut string("example.com"))
            .is_err()
    );
}

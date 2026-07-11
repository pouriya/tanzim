use std::error::Error as StdError;
use std::path::PathBuf;
use tanzim_testing::environment::{Error, run};

#[test]
fn error_display_and_source() {
    let error = Error::NotRelative {
        path: PathBuf::from("/abs"),
    };
    assert!(error.to_string().contains("relative"));
    assert!(StdError::source(&error).is_none());

    let inner = std::io::Error::new(std::io::ErrorKind::NotFound, "nope");
    let error = Error::Io {
        action: String::from("create the file"),
        path: Some(PathBuf::from("x")),
        source: inner,
    };
    assert!(error.to_string().contains("create the file"));
    assert!(StdError::source(&error).is_some());
    assert!(format!("{error:#}").contains("nope"));
}

// All sandbox behavior lives in one test so that only a single test mutates the process-global
// working directory / environment; parallel tests would otherwise race despite the lock.
#[test]
fn sandbox_lifecycle() {
    let before = std::env::current_dir().unwrap();

    // SAFETY: this is the only test that mutates the process environment.
    unsafe { std::env::set_var("TANZIM_TESTING_PRE", "keep") };

    let sandbox = run(|env| {
        let directory = env.directory().unwrap().to_path_buf();
        assert_eq!(std::env::current_dir().unwrap(), directory);

        env.create_file("empty.txt")?;
        assert!(std::fs::metadata("empty.txt").unwrap().is_file());

        env.write_file("cfg/app.json", b"{\"port\":8080}")?;
        assert_eq!(
            std::fs::read_to_string("cfg/app.json").unwrap(),
            "{\"port\":8080}"
        );
        env.write_file("cfg/app.json", b"{}")?;
        assert_eq!(std::fs::read_to_string("cfg/app.json").unwrap(), "{}");

        env.create_directory("logs")?;
        assert!(std::fs::metadata("logs").unwrap().is_dir());

        assert!(matches!(
            env.create_file(if cfg!(windows) {
                r"C:\etc\passwd"
            } else {
                "/etc/passwd"
            }),
            Err(Error::NotRelative { .. })
        ));
        assert!(matches!(
            env.create_file("../escape.txt"),
            Err(Error::Escapes { .. })
        ));

        env.set_env("TANZIM_TESTING_INNER", "1")?;
        assert_eq!(std::env::var("TANZIM_TESTING_INNER").unwrap(), "1");
        env.clear_env();
        assert!(std::env::var("TANZIM_TESTING_PRE").is_err());

        Ok(directory)
    })
    .unwrap();

    assert_eq!(std::env::current_dir().unwrap(), before);
    assert!(!sandbox.exists());
    assert!(std::env::var("TANZIM_TESTING_INNER").is_err());
    assert_eq!(std::env::var("TANZIM_TESTING_PRE").unwrap(), "keep");

    let converted: Result<(), Error> = run(|_env| {
        let io = std::io::Error::other("boom");
        Err(io)?;
        Ok(())
    });
    assert!(matches!(converted, Err(Error::Other(_))));

    let panicked = std::panic::catch_unwind(|| {
        let _ = run(|_env| -> Result<(), Error> { panic!("boom") });
    });
    assert!(panicked.is_err());
    assert_eq!(std::env::current_dir().unwrap(), before);

    // SAFETY: this is the only test that mutates the process environment.
    unsafe { std::env::remove_var("TANZIM_TESTING_PRE") };
}

use tanzim::merger::{Entries, EntryName, EntryNameRef};

#[test]
fn entry_name_display_prints_root_in_angle_brackets() {
    assert_eq!(EntryName::root().to_string(), "<root>");
    assert_eq!(EntryName::named("db").to_string(), "db");
    assert_eq!(EntryNameRef::Root.to_string(), "<root>");
    assert_eq!(EntryNameRef::Named("db").to_string(), "db");
}

#[test]
fn entry_name_as_ref_and_to_owned_round_trip() {
    let named = EntryName::named("app");
    match named.as_ref() {
        EntryNameRef::Named("app") => {}
        other => panic!("unexpected name: {other}"),
    }
    assert_eq!(EntryNameRef::Root.to_owned(), EntryName::Root);
    assert_eq!(
        EntryNameRef::Named("web").to_owned(),
        EntryName::named("web")
    );
}

#[test]
fn entries_root_and_named_accessors() {
    let mut entries = Entries::new();
    assert!(entries.is_empty());

    assert_eq!(entries.insert_root(1), None);
    assert_eq!(entries.insert_named("db", 2), None);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries.root(), Some(&1));
    assert_eq!(entries.named("db"), Some(&2));
    assert!(entries.contains_root());
    assert!(entries.contains_named("db"));
    assert!(entries.contains(&EntryName::Root));
    assert!(entries.contains(&EntryName::named("db")));
    assert_eq!(entries.get(&EntryName::Root), Some(&1));
    assert_eq!(entries.get(&EntryName::named("db")), Some(&2));

    *entries.root_mut().unwrap() = 10;
    *entries.named_mut("db").unwrap() = 20;
    assert_eq!(entries.root(), Some(&10));
    assert_eq!(entries.named("db"), Some(&20));

    assert_eq!(entries.insert(EntryName::named("cache"), 3), None);
    assert_eq!(entries.named("cache"), Some(&3));
}

#[test]
fn entries_remove_and_replace() {
    let mut entries = Entries::new();
    entries.insert_root(1);
    entries.insert_named("db", 2);

    assert_eq!(entries.insert_root(9), Some(1));
    assert_eq!(entries.insert_named("db", 8), Some(2));
    assert_eq!(entries.remove_root(), Some(9));
    assert_eq!(entries.remove_named("db"), Some(8));
    assert!(entries.is_empty());

    entries.insert_root(1);
    entries.insert_named("db", 2);
    assert_eq!(entries.remove(&EntryName::Root), Some(1));
    assert_eq!(entries.remove(&EntryName::named("db")), Some(2));
}

#[test]
fn entries_iter_and_keys_use_entry_name_ref() {
    let mut entries = Entries::new();
    entries.insert_root(1);
    entries.insert_named("db", 2);

    let mut saw_root = false;
    let mut saw_db = false;
    for name in entries.keys() {
        match name {
            EntryNameRef::Root => saw_root = true,
            EntryNameRef::Named("db") => saw_db = true,
            other => panic!("unexpected key: {other}"),
        }
    }
    assert!(saw_root && saw_db);

    saw_root = false;
    saw_db = false;
    for (name, value) in entries.iter() {
        match name {
            EntryNameRef::Root => {
                assert_eq!(value, &1);
                saw_root = true;
            }
            EntryNameRef::Named("db") => {
                assert_eq!(value, &2);
                saw_db = true;
            }
            other => panic!("unexpected key: {other}"),
        }
    }
    assert!(saw_root && saw_db);

    for (name, value) in entries.iter_mut() {
        match name {
            EntryNameRef::Root => *value = 11,
            EntryNameRef::Named("db") => *value = 22,
            other => panic!("unexpected key: {other}"),
        }
    }
    assert_eq!(entries.root(), Some(&11));
    assert_eq!(entries.named("db"), Some(&22));
}

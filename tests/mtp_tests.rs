use teleport::mtp::session::{join_remote_path, normalize_remote_path};

#[test]
fn normalize_remote_path_handles_root_and_children() {
    assert_eq!(normalize_remote_path("/"), "/");
    assert_eq!(normalize_remote_path("Internal storage"), "/Internal storage");
    assert_eq!(normalize_remote_path("//SD card//"), "/SD card");
}

#[test]
fn join_remote_path_builds_nested_paths() {
    assert_eq!(join_remote_path("/", "Internal storage"), "/Internal storage");
    assert_eq!(
        join_remote_path("/Internal storage", "DCIM"),
        "/Internal storage/DCIM"
    );
    assert_eq!(
        join_remote_path("/Internal storage/", "//DCIM//"),
        "/Internal storage/DCIM"
    );
}

#[test]
fn path_helpers_handle_edge_cases() {
    assert_eq!(normalize_remote_path(""), "/");
    assert_eq!(normalize_remote_path("   "), "/");
    assert_eq!(normalize_remote_path("///"), "/");
    assert_eq!(join_remote_path("/", "/"), "/");
    assert_eq!(
        join_remote_path("Internal storage", "DCIM/"),
        "/Internal storage/DCIM"
    );
}

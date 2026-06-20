/// Architecture guard tests that enforce clean-architecture dependency rules.
///
/// The dependency direction must be: domain → application → ports → adapters.
/// Domain must not depend on application, ports, or adapters.
/// Application must not depend on adapters.
/// Ports must not depend on adapters.
#[cfg(test)]
mod architecture_guard_tests {
    use std::fs;

    fn read_all_rust_files(dir: &str) -> Vec<String> {
        let mut contents = Vec::new();
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "rs") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        contents.push(content);
                    }
                }
            }
        }
        contents
    }

    #[test]
    fn domain_does_not_import_application_layer() {
        let domain_files = read_all_rust_files("src/domain");
        for content in &domain_files {
            assert!(
                !content.contains("use crate::application"),
                "domain layer must not import application layer"
            );
        }
    }

    #[test]
    fn domain_does_not_import_ports_layer() {
        let domain_files = read_all_rust_files("src/domain");
        for content in &domain_files {
            assert!(
                !content.contains("use crate::ports"),
                "domain layer must not import ports layer"
            );
        }
    }

    #[test]
    fn domain_does_not_import_adapters_layer() {
        let domain_files = read_all_rust_files("src/domain");
        for content in &domain_files {
            assert!(
                !content.contains("use crate::adapters"),
                "domain layer must not import adapters layer"
            );
        }
    }

    #[test]
    fn application_does_not_import_adapters_layer() {
        let application_files = read_all_rust_files("src/application");
        for content in &application_files {
            assert!(
                !content.contains("use crate::adapters"),
                "application layer must not import adapters layer"
            );
        }
    }

    #[test]
    fn ports_does_not_import_adapters_layer() {
        let ports_files = read_all_rust_files("src/ports");
        for content in &ports_files {
            assert!(
                !content.contains("use crate::adapters"),
                "ports layer must not import adapters layer"
            );
        }
    }

    #[test]
    fn domain_does_not_import_tauri() {
        let domain_files = read_all_rust_files("src/domain");
        for content in &domain_files {
            assert!(
                !content.contains("tauri"),
                "domain layer must not depend on Tauri framework"
            );
        }
    }

    #[test]
    fn domain_does_not_import_tokio() {
        let domain_files = read_all_rust_files("src/domain");
        for content in &domain_files {
            assert!(
                !content.contains("tokio"),
                "domain layer must not depend on async runtime (tokio)"
            );
        }
    }
}

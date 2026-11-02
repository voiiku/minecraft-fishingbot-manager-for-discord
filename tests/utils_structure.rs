//! 检查公共工具模块的职责边界。

use std::fs;
use std::path::Path;

use syn::visit::{self, Visit};

#[derive(Default)]
struct TypeDefinitions {
    names: Vec<String>,
}

impl<'ast> Visit<'ast> for TypeDefinitions {
    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        self.names.push(format!("struct {}", item.ident));
        visit::visit_item_struct(self, item);
    }

    fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
        self.names.push(format!("enum {}", item.ident));
        visit::visit_item_enum(self, item);
    }
}

fn inspect_file(path: &Path, violations: &mut Vec<String>) {
    let source = fs::read_to_string(path).expect("read utils source");
    let syntax = syn::parse_file(&source).expect("parse utils source");
    let mut types = TypeDefinitions::default();
    types.visit_file(&syntax);
    violations.extend(
        types
            .names
            .into_iter()
            .map(|name| format!("{}: {name}", path.display())),
    );
}

fn inspect_directory(dir: &Path, violations: &mut Vec<String>) {
    for entry in fs::read_dir(dir).expect("read utils directory") {
        let path = entry.expect("read utils entry").path();
        if path.is_dir() {
            inspect_directory(&path, violations);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            inspect_file(&path, violations);
        }
    }
}

#[test]
fn utils_contains_no_struct_or_enum_definitions() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut violations = Vec::new();
    inspect_file(&src.join("utils.rs"), &mut violations);
    inspect_directory(&src.join("utils"), &mut violations);
    assert!(violations.is_empty(), "{}", violations.join("\n"));
}

#[test]
fn nested_type_definitions_are_detected() {
    let syntax = syn::parse_file("mod inner { fn example() { struct Value; enum State { A } } }")
        .expect("parse fixture");
    let mut types = TypeDefinitions::default();
    types.visit_file(&syntax);
    assert_eq!(types.names, ["struct Value", "enum State"]);
}

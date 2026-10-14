// Adversarial Rust file: raw strings containing keywords and tricky syntax

pub fn calculate_pi() -> f64 {
    // "fn fake_function() {}" inside string:
    let _fake_code = r#"
        pub fn injected_code() {
            struct FakeStruct;
        }
    "#;
    std::f64::consts::PI
}

pub struct ComplexUnicodeStruct {
    pub label: &'static str,
}

impl ComplexUnicodeStruct {
    pub fn new() -> Self {
        Self { label: "Unicode: 🦀 日本語 🚀" }
    }
}

use serde::{Serialize, Serializer, ser::SerializeStruct};
use std::collections::HashSet;
use std::fmt;

use crate::Checker;
use crate::macros::implement_metric_trait;
use crate::node::Node;
use crate::*;

/// The `Unsafe` metric suite.
///
/// This metric counts the number of lines that contain structurally
/// unsafe code patterns, such as `unsafe` blocks, pointer arithmetic,
/// unchecked array indexing, or non-null assertions.
#[derive(Debug, Clone, Default)]
pub struct Stats {
    unsafe_lines: HashSet<usize>,
}

impl Serialize for Stats {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut st = serializer.serialize_struct("unsafe", 1)?;
        st.serialize_field("unsafe_lines", &self.count())?;
        st.end()
    }
}

impl fmt::Display for Stats {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "unsafe_lines: {}", self.count())
    }
}

impl Stats {
    /// Merges a second `Unsafe` metric into the first one
    pub fn merge(&mut self, other: &Stats) {
        for line in &other.unsafe_lines {
            self.unsafe_lines.insert(*line);
        }
    }

    /// Returns the total number of lines containing unsafe patterns.
    #[inline(always)]
    pub fn count(&self) -> f64 {
        self.unsafe_lines.len() as f64
    }
}

pub trait SafeCheck
where
    Self: Checker,
{
    fn compute(node: &Node, stats: &mut Stats);
}

// -----------------------------------------------------------------------------
// Language-Specific Implementations
// -----------------------------------------------------------------------------

impl SafeCheck for RustCode {
    fn compute(node: &Node, stats: &mut Stats) {
        use crate::Rust::*;
        // In Rust, we explicitly look for unsafe blocks and raw pointers.
        if matches!(node.kind_id().into(), UnsafeBlock | PointerType) {
            stats.unsafe_lines.insert(node.start_row());
        }
    }
}

impl SafeCheck for CppCode {
    fn compute(node: &Node, stats: &mut Stats) {
        use crate::Cpp::*;
        // C and C++ memory unsafety typically stems from pointers, unchecked
        // array indexing, manual memory management, and forced casting.
        if matches!(
            node.kind_id().into(),
            PointerExpression
                | SubscriptExpression
                | NewExpression
                | DeleteExpression
                | CastExpression
        ) {
            stats.unsafe_lines.insert(node.start_row());
        }
    }
}

impl SafeCheck for TypescriptCode {
    fn compute(node: &Node, stats: &mut Stats) {
        use crate::Typescript::*;
        // Bypassing the strict null checker is the primary structural "unsafe" pattern.
        if matches!(node.kind_id().into(), NonNullExpression) {
            stats.unsafe_lines.insert(node.start_row());
        }
    }
}

impl SafeCheck for TsxCode {
    fn compute(node: &Node, stats: &mut Stats) {
        use crate::Tsx::*;
        if matches!(node.kind_id().into(), NonNullExpression) {
            stats.unsafe_lines.insert(node.start_row());
        }
    }
}

impl SafeCheck for KotlinCode {
    fn compute(node: &Node, stats: &mut Stats) {
        use crate::Kotlin::*;
        // Kotlin's `!!` operator explicitly bypasses null safety
        if matches!(node.kind_id().into(), BANGBANG) {
            stats.unsafe_lines.insert(node.start_row());
        }
    }
}

impl SafeCheck for HaskellCode {
    fn compute(node: &Node, stats: &mut Stats) {
        use crate::Haskell::*;
        // Haskell's structural "unsafe" boundary is the FFI.
        if matches!(node.kind_id().into(), ForeignImport | ForeignExport) {
            stats.unsafe_lines.insert(node.start_row());
        }
    }
}

impl SafeCheck for GoCode {
    fn compute(node: &Node, stats: &mut Stats) {
        use crate::Go::*;
        // Go restricts pointers heavily, but using them still represents
        // a lower-level, potentially risky memory access pattern (especially with CGo).
        if matches!(node.kind_id().into(), PointerType) {
            stats.unsafe_lines.insert(node.start_row());
        }
    }
}

impl SafeCheck for SwiftCode {
    fn compute(node: &Node, stats: &mut Stats) {
        use crate::Swift::*;

        let kind = node.kind_id().into();

        if matches!(
            kind,
            // Catches `as!`, `try!`, and `unowned(unsafe)`
            AsBANG | AsBang | FakeTryBang | UnownedLPARENunsafeRPAREN
        ) {
            stats.unsafe_lines.insert(node.start_row());
        } else if kind == PostfixUnaryOperator {
            // Catches forced unwrapping of optionals (e.g., `value!`)
            // We check the children to ensure the postfix operator is actually a bang `!`
            for child in node.children() {
                if matches!(child.kind_id().into(), BANG | BANG2 | Bang | BangCustom) {
                    stats.unsafe_lines.insert(node.start_row());
                    break;
                }
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Default Implementations for Managed Languages
// -----------------------------------------------------------------------------

implement_metric_trait!(
    SafeCheck,
    PythonCode,
    MozjsCode,
    JavascriptCode,
    JavaCode,
    ScalaCode,
    PreprocCode,
    CcommentCode
);

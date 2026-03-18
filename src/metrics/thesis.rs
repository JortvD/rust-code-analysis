use serde::{Serialize, Serializer, ser::SerializeStruct};
use std::collections::HashSet;
use std::fmt;

use crate::Checker;
use crate::macros::implement_metric_trait;
use crate::node::Node;
use crate::*;

/// The `Thesis`-specific metric suite.
///
/// This metric counts the number of lines that contain structurally
/// unsafe code patterns, such as `unsafe` blocks, pointer arithmetic,
/// unchecked array indexing, or non-null assertions.
#[derive(Debug, Clone)]
pub struct Stats {
    unsafe_lines: HashSet<usize>,
    assertions: usize,
}

impl Default for Stats {
    fn default() -> Self {
        Stats {
            unsafe_lines: HashSet::new(),
            assertions: 0,
        }
    }
}

impl Serialize for Stats {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut st = serializer.serialize_struct("thesis", 2)?;
        st.serialize_field("unsafe_lines", &self.count())?;
        st.serialize_field("assertions", &self.assertions)?;
        st.end()
    }
}

impl fmt::Display for Stats {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "unsafe_lines: {}, assertions: {}", self.count(), self.assertions)
    }
}

impl Stats {
    /// Merges a second `Unsafe` metric into the first one
    pub fn merge(&mut self, other: &Stats) {
        for line in &other.unsafe_lines {
            self.unsafe_lines.insert(*line);
        }
        self.assertions += other.assertions;
    }

    /// Returns the total number of lines containing unsafe patterns.
    #[inline(always)]
    pub fn count(&self) -> f64 {
        self.unsafe_lines.len() as f64
    }

    pub fn assertions(&self) -> f64 {
        self.assertions as f64
    }

    pub fn unsafe_lines(&self) -> &HashSet<usize> {
        &self.unsafe_lines
    }
}

pub trait Thesis
where
    Self: Checker,
{
    fn compute(node: &Node, stats: &mut Stats);
}

#[inline(always)]
fn add_unsafe_lines(stats: &mut Stats, start: usize, end: usize) {
    for line in start..=end {
        stats.unsafe_lines.insert(line);
    }
}

#[inline(always)]
fn check_assert_child(node: &Node, field: &str, stats: &mut Stats, predicate: impl Fn(&str) -> bool) {
    if let Some(name_node) = node.child_by_field_name(field).or_else(|| node.child(0)) {
        if let Some(name) = name_node.utf8_text(node.2) {
            if predicate(name) {
                stats.assertions += 1;
            }
        }
    }
}

impl Thesis for RustCode {
    fn compute(node: &Node, stats: &mut Stats) {
        use crate::Rust::*;

        match node.kind_id().into() {
            UnsafeBlock => add_unsafe_lines(stats, node.start_row(), node.end_row()),
            ImplItem | TraitItem => {
                let mut children = node.children();
                if children.any(|child| Into::<Rust>::into(child.kind_id()) == Unsafe) {
                    add_unsafe_lines(stats, node.start_row(), node.end_row());
                }
            }
            FunctionModifiers => {
                let mut children = node.children();
                if children.any(|child| Into::<Rust>::into(child.kind_id()) == Unsafe) {
                    let parent = node.parent().unwrap();
                    add_unsafe_lines(stats, parent.start_row(), parent.end_row());
                }
            }
            _ => {}
        }

        match &node.kind_id().into() {
            MacroInvocation => check_assert_child(node, "macro", stats, |m| {
                matches!(m.trim(), "assert" | "assert_eq" | "assert_ne" | "debug_assert" | "debug_assert_eq" | "debug_assert_ne" | "assert_debug_snapshot" | "assert_snapshot" | "assert_json_snapshot")
            }),
            Attribute => {
                if let Some(text) = node.utf8_text(node.2) {
                    if text.contains("should_panic") {
                        stats.assertions += 1;
                    }
                }
            },
            CallExpression => check_assert_child(node, "function", stats, |f| f == "assert_that"),
            _ => {}
        }
    }
}

impl Thesis for CppCode {
    fn compute(node: &Node, stats: &mut Stats) {
        use crate::Cpp::*;
        
        match &node.kind_id().into() {
            PointerExpression | SubscriptExpression | NewExpression | DeleteExpression | CastExpression => {
                add_unsafe_lines(stats, node.start_row(), node.end_row());
            }
            CallExpression | CallExpression2 => {
                if let Some(function_node) = node.child_by_field_name("function") && let Some(function_name) = function_node.utf8_text(node.2) {
                    if matches!(function_name.trim(), "malloc" | "calloc" | "realloc" | "free" | "memcpy" | "memmove" | "memset" | "strcpy" | "strncpy" | "sprintf" | "vsprintf" | "snprintf" | "vsnprintf" | "strcat" | "strncat" | "gets" | "scanf" | "sscanf" | "fscanf" | "bcopy" | "bzero" | "strdup" | "strndup" | "memcmp" | "strlen" | "posix_memalign" | "valloc" | "alloca") {
                        add_unsafe_lines(stats, node.start_row(), node.end_row());
                    }
                }
            }
            _ => {}
        }

        match &node.kind_id().into() {
            CallExpression | CallExpression2 => check_assert_child(node, "function", stats, |f| {
                matches!(f.trim(), 
                    "ASSERT" | "ASSERT_EQ" | "ASSERT_NE" | "EXPECT_EQ" | "EXPECT_NE" | "REQUIRE" | "CHECK" | "ASSERT_TRUE" | "ASSERT_FALSE" | "ASSERT_NULL" | "ASSERT_NOTNULL" |
                    "TEST_ASSERT_EQUAL_INT" | "TEST_ASSERT_EQUAL_STRING" | "TEST_ASSERT_TRUE" | "TEST_ASSERT_FALSE" | "TEST_ASSERT_NULL" | "TEST_ASSERT_NOT_NULL" |
                    "assert" | "assert_eq" | "assert_ne" | "assert_true" | "assert_false" | "assert_null" | "assert_notnull"
                )
            }),
            _ => {}
        }
    }
}

impl Thesis for TypescriptCode {
    fn compute(node: &Node, stats: &mut Stats) {
        use crate::Typescript::*;

        match &node.kind_id().into() {
            CallExpression | CallExpression2 | CallExpression3 | CallExpression4 => check_assert_child(node, "function", stats, |f| f == "expect" || f == "assert"),
            MemberExpression | MemberExpression2 | MemberExpression3 | MemberExpression4 => {
                check_assert_child(node, "object", stats, |o| o == "assert" || o == "should");
                check_assert_child(node, "property", stats, |p| p == "assert" || p == "should");
            }
            _ => {}
        }
    }
}

impl Thesis for TsxCode {
    fn compute(node: &Node, stats: &mut Stats) {
        use crate::Tsx::*;

        match &node.kind_id().into() {
            CallExpression | CallExpression2 | CallExpression3 | CallExpression4 => check_assert_child(node, "function", stats, |f| f == "expect" || f == "assert"),
            MemberExpression | MemberExpression2 | MemberExpression3 | MemberExpression4 => {
                check_assert_child(node, "object", stats, |o| o == "assert" || o == "should");
                check_assert_child(node, "property", stats, |p| p == "assert" || p == "should");
            }
            _ => {}
        }
    }
}

impl Thesis for HaskellCode {
    fn compute(node: &Node, stats: &mut Stats) {
        use crate::Haskell::*;
        
        match &node.kind_id().into() {
            Apply | Apply2 | Apply3 | Apply4 => check_assert_child(node, "function", stats, |f| {
                f.starts_with("unsafe")
            }),
            Infix | Infix2 | Infix3 | Infix4 | Infix5 | Infix6 | Infix7 | Infix8 => check_assert_child(node, "left_operand", stats, |o| {
                o.starts_with("unsafe")
            }),
            _ => {}
        }

        match &node.kind_id().into() {
            Apply | Apply2 | Apply3 | Apply4 => check_assert_child(node, "function", stats, |f| {
                matches!(f, "assert" | "assertEqual" | "assertBool" | "shouldBe" | "shouldSatisfy")
            }),
            Infix | Infix2 | Infix3 | Infix4 | Infix5 | Infix6 | Infix7 | Infix8 => check_assert_child(node, "operator", stats, |name| {
                matches!(name.trim(), "`shouldBe`" | "`shouldSatisfy`" | "`shouldThrow`")
            }),
            _ => {}
        }
    }
}

impl Thesis for GoCode {
    fn compute(node: &Node, stats: &mut Stats) {
        use crate::Go::*;

        match &node.kind_id().into() {
            SelectorExpression => {
                if let Some(operand) = node.child_by_field_name("operand") && let Some(operand_text) = operand.utf8_text(node.2) &&operand_text == "unsafe" {
                    stats.unsafe_lines.insert(node.start_row());
                }
            },
            _ => {}
        }

        match &node.kind_id().into() {
            SelectorExpression => {
                if let Some(operand) = node.child_by_field_name("operand").and_then(|n| n.utf8_text(node.2)) && operand == "t" {
                    check_assert_child(node, "field", stats, |f| {
                        matches!(f.trim(), "Errorf" | "Fatalf" | "Panicf" | "Error" | "Fatal" | "Panic")
                    });
                }
                check_assert_child(node, "operand", stats, |f| {
                    matches!(f.trim(), "assert")
                });
            }
            _ => {}
        }
    }
}

impl Thesis for SwiftCode {
    fn compute(node: &Node, stats: &mut Stats) {
        use crate::Swift::*;

        match &node.kind_id().into() {
            TypeIdentifier | CallExpression | CallExpression2 => {
                if let Some(text) = node.utf8_text(node.2) && text.starts_with("Unsafe") {
                    stats.unsafe_lines.insert(node.start_row());
                }
            }
            SimpleIdentifier => {
                if let Some(text) = node.utf8_text(node.2) && text.starts_with("withUnsafe") {
                    stats.unsafe_lines.insert(node.start_row());
                }
            }
            _ => {}
        }

        match &node.kind_id().into() {
            CallExpression | CallExpression2 => check_assert_child(node, "function", stats, |f| {
                matches!(f.trim(), 
                    "XCTAssert" | "XCTAssertEqual" | "XCTAssertTrue" | "XCTAssertFalse" | "XCTAssertNil" | "XCTAssertNotNil" | "XCTAssertThrowsError" | "XCTAssertNoThrow"
                )
            }),
            MacroInvocation => {
                if let Some(text) = node.utf8_text(node.2) {
                    if text.contains("expect") {
                        stats.assertions += 1;
                    }
                }
            },
            _ => {}
        }
    }
}

impl Thesis for PythonCode {
    fn compute(node: &Node, stats: &mut Stats) {
        use crate::Python::*;

        match &node.kind_id().into() {
            Call => check_assert_child(node, "function", stats, |f| 
                f == "assert" || 
                f == "assertTrue" || 
                f == "assertEquals" || 
                f == "assertFalse" || 
                f == "assertIsNone" || 
                f == "assertIsNotNone" || 
                f == "assertIn" || 
                f == "assertNotIn" || 
                f == "assertRaises"
            ),
            AssertStatement => stats.assertions += 1,
            _ => {}
        }
    }
}

impl Thesis for JavascriptCode {
    fn compute(node: &Node, stats: &mut Stats) {
        use crate::Javascript::*;

        match &node.kind_id().into() {
            CallExpression | CallExpression2 => check_assert_child(node, "function", stats, |f| f == "expect" || f == "assert"),
            MemberExpression | MemberExpression2 | MemberExpression3 => {
                check_assert_child(node, "object", stats, |o| o == "assert" || o == "should");
                check_assert_child(node, "property", stats, |p| p == "assert" || p == "should");
            }
            _ => {}
        }
    }
}

impl Thesis for JavaCode {
    fn compute(node: &Node, stats: &mut Stats) {
        use crate::Java::*;

        match &node.kind_id().into() {
            MethodInvocation => {
                check_assert_child(node, "name", stats, |name| {
                    matches!(name.trim(), 
                        "assertEquals" | "assertTrue" | "assertFalse" | "assertNull" | "assertNotNull" | "assertSame" | "assertNotSame" | "assertArrayEquals" | "assertThat" |
                        "isTrue" | "isFalse" | "isNull" | "isNotNull" | "sameInstance" | "notSameInstance" | "arrayEquals" | "that"
                    )
                });
                check_assert_child(node, "object", stats, |object| {
                    object == "Assert" || object == "Assertions"
                });
            },
            AssertStatement => stats.assertions += 1,
            _ => {}
        }
    }
}

impl Thesis for ScalaCode {
    fn compute(node: &Node, stats: &mut Stats) {
        use crate::Scala::*;

        match &node.kind_id().into() {
            CallExpression => check_assert_child(node, "function", stats, |f| {
                matches!(f.trim(), "assert" | "assertEquals" | "assertNotEquals" | "assertTrue" | "assertFalse" | "assertNull" | "assertNotNull" | "assertSame" | "assertNotSame" | "assertArrayEquals" | "assertThat")
            }),
            InfixExpression => check_assert_child(node, "operator", stats, |op| {
                matches!(op.trim(), 
                    "shouldBe" | "shouldEqual" | "shouldNotEqual" | 
                    "shouldBeGreaterThan" | "shouldBeLessThan" | 
                    "shouldBeGreaterThanOrEqual" | "shouldBeLessThanOrEqual" |
                    "shouldBeLike" | "shouldMatch" |
                    "shouldStartWith" | "shouldEndWith" |
                    "shouldContain" | "shouldNotContain" |
                    "shouldBeEmpty" | "shouldNotBeEmpty" |
                    "shouldExist" | "shouldNotExist" |
                    "shouldBeDefined" | "shouldNotBeDefined" |
                    "must"
                )
            }),
            GenericFunction => check_assert_child(node, "function", stats, |f| f == "assertThrows"),
            _ => {}
        }
    }
}

implement_metric_trait!(
    Thesis,
    MozjsCode,
    PreprocCode,
    CcommentCode,
    KotlinCode
);

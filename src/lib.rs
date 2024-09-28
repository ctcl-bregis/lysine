#[macro_use]
mod macros;
mod builtins;
mod context;
mod errors;
mod filter_utils;
mod parser;
mod renderer;
mod template;
mod lysine;
mod utils;

pub use crate::builtins::filters::Filter;
pub use crate::builtins::functions::Function;
pub use crate::builtins::testers::Test;
pub use crate::context::Context;
pub use crate::errors::{Error, ErrorKind, Result};
// Template, dotted_pointer and get_json_pointer are meant to be used internally only but is exported for test/bench.
#[doc(hidden)]
pub use crate::context::dotted_pointer;
#[doc(hidden)]
#[allow(deprecated)]
pub use crate::context::get_json_pointer;
#[doc(hidden)]
pub use crate::template::Template;
pub use crate::lysine::Lysine;
pub use crate::utils::escape_html;
// Re-export Value and other useful things from serde
// so apps/tools can encode data in Tera types
pub use serde_json::value::{from_value, to_value, Map, Number, Value};

// Exposes the AST if one needs it but changing the AST is not considered
// a breaking change so it isn't public
#[doc(hidden)]
pub use crate::parser::ast;

// Re-export some helper fns useful to write filters/fns/tests
pub mod helpers {
    // Functions helping writing tests
    pub mod tests {
        pub use crate::builtins::testers::{extract_string, number_args_allowed, value_defined};
    }
}

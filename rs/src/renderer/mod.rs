mod square_brackets;
#[cfg(test)]
mod tests;

mod call_stack;
mod for_loop;
mod macros;
mod processor;
mod stack_frame;

use std::io::Write;

use self::processor::Processor;
use crate::errors::Result;
use crate::template::Template;
use crate::lysine::Lysine;
use crate::utils::buffer_to_string;
use crate::Context;

// Given a `Lysine` and reference to `Template` and a `Context`, renders text
#[derive(Debug)]
pub struct Renderer<'a> {
    // Template to render
    template: &'a Template,
    // Houses other templates, filters, global functions, etc
    lysine: &'a Lysine,
    // Read-only context to be bound to template˝
    context: &'a Context,
    // If set rendering should be escaped
    should_escape: bool,
}

impl<'a> Renderer<'a> {
    // Create a new `Renderer`
    #[inline]
    pub fn new(template: &'a Template, lysine: &'a Lysine, context: &'a Context) -> Renderer<'a> {
        let should_escape = lysine.autoescape_suffixes.iter().any(|ext| {
            // We prefer a `path` if set, otherwise use the `name`
            if let Some(ref p) = template.path {
                return p.ends_with(ext);
            }
            template.name.ends_with(ext)
        });

        Renderer { template, lysine, context, should_escape }
    }

    // Combines the context with the Template to generate the end result
    pub fn render(&self) -> Result<String> {
        let mut output = Vec::with_capacity(2000);
        self.render_to(&mut output)?;
        buffer_to_string(|| "converting rendered buffer to string".to_string(), output)
    }

    // Combines the context with the Template to write the end result to output
    pub fn render_to(&self, mut output: impl Write) -> Result<()> {
        let mut processor =
            Processor::new(self.template, self.lysine, self.context, self.should_escape);

        processor.render(&mut output)
    }
}

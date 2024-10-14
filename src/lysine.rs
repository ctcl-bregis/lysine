use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use std::sync::Arc;

use globwalk::glob_builder;

use crate::builtins::filters::{array, common, number, object, string, Filter};
use crate::builtins::functions;
use crate::builtins::functions::Function;
use crate::builtins::testers::{self, Test};
use crate::context::Context;
use crate::errors::{Error, Result};
use crate::renderer::Renderer;
use crate::template::Template;
use crate::utils::escape_html;

// Default template name used for `Lysine::render_str` and `Lysine::one_off`.
const ONE_OFF_TEMPLATE_NAME: &str = "__lysine_one_off";

// The escape function type definition
pub type EscapeFn = fn(&str) -> String;

#[derive(Clone)]
pub struct Lysine {
    // The glob used in `Lysine::new`, None if Lysine was instantiated differently
    #[doc(hidden)]
    glob: Option<String>,
    #[doc(hidden)]
    pub templates: HashMap<String, Template>,
    #[doc(hidden)]
    pub filters: HashMap<String, Arc<dyn Filter>>,
    #[doc(hidden)]
    pub testers: HashMap<String, Arc<dyn Test>>,
    #[doc(hidden)]
    pub functions: HashMap<String, Arc<dyn Function>>,
    #[doc(hidden)]
    pub autoescape_suffixes: Vec<&'static str>,
    #[doc(hidden)]
    escape_fn: EscapeFn,
}

impl Lysine {
    fn create(dir: &str, parse_only: bool) -> Result<Lysine> {
        if dir.find('*').is_none() {
            return Err(Error::msg(format!(
                "Lysine expects a glob as input, no * were found in `{}`",
                dir
            )));
        }

        let mut lysine = Lysine {
            glob: Some(dir.to_string()),
            templates: HashMap::new(),
            filters: HashMap::new(),
            functions: HashMap::new(),
            testers: HashMap::new(),
            autoescape_suffixes: vec![".lisc", ".lism", ".lish"],
            escape_fn: escape_html,
        };

        lysine.load_from_glob()?;
        if !parse_only {
            lysine.build_inheritance_chains()?;
            lysine.check_macro_files()?;
        }
        lysine.register_lysine_filters();
        lysine.register_lysine_testers();
        lysine.register_lysine_functions();
        Ok(lysine)
    }

    pub fn new(dir: &str) -> Result<Lysine> {
        Self::create(dir, false)
    }

    pub fn parse(dir: &str) -> Result<Lysine> {
        Self::create(dir, true)
    }

    // Loads all the templates found in the glob that was given to [`Lysine::new`].
    fn load_from_glob(&mut self) -> Result<()> {
        let glob = match &self.glob {
            Some(g) => g,
            None => return Err(Error::msg("Lysine can only load from glob if a glob is provided")),
        };

        // We want to preserve templates that have been added through
        // Lysine::extend so we only keep those
        self.templates = self
            .templates
            .iter()
            .filter(|&(_, t)| t.from_extend)
            .map(|(n, t)| (n.clone(), t.clone())) // TODO: avoid that clone
            .collect();

        let mut errors = String::new();

        // Need to canonicalize the glob path because globwalk always returns
        // an empty list for paths starting with `./` or `../`.
        // See https://github.com/Keats/lysine/issues/574 for the Lysine discussion
        // and https://github.com/Gilnaa/globwalk/issues/28 for the upstream issue.
        let (parent_dir, glob_end) = glob.split_at(glob.find('*').unwrap());
        let parent_dir = match std::fs::canonicalize(parent_dir) {
            Ok(d) => d,
            // If canonicalize fails, just abort it and resume with the given path.
            // Consumers expect invalid globs to just return the empty set instead of failing.
            // See https://github.com/Keats/lysine/issues/819#issuecomment-1480392230
            Err(_) => std::path::PathBuf::from(parent_dir),
        };
        let dir = parent_dir.join(glob_end).into_os_string().into_string().unwrap();

        // We are parsing all the templates on instantiation
        for entry in glob_builder(&dir)
            .follow_links(true)
            .build()
            .unwrap()
            .filter_map(std::result::Result::ok)
        {
            let mut path = entry.into_path();
            // We only care about actual files
            if path.is_file() {
                if path.starts_with("./") {
                    path = path.strip_prefix("./").unwrap().to_path_buf();
                }

                let filepath = path
                    .strip_prefix(&parent_dir)
                    .unwrap()
                    .to_string_lossy()
                    // unify on forward slash
                    .replace('\\', "/");

                if let Err(e) = self.add_file(Some(&filepath), path) {
                    use std::error::Error;

                    errors += &format!("\n- {}", e);
                    let mut cause = e.source();
                    while let Some(e) = cause {
                        errors += &format!("\n{}", e);
                        cause = e.source();
                    }
                }
            }
        }

        if !errors.is_empty() {
            return Err(Error::msg(errors));
        }

        Ok(())
    }

    // Add a template from a path: reads the file and parses it.
    // This will return an error if the template is invalid and doesn't check the validity of
    // inheritance chains.
    fn add_file<P: AsRef<Path>>(&mut self, name: Option<&str>, path: P) -> Result<()> {
        let path = path.as_ref();
        let tpl_name = name.unwrap_or_else(|| path.to_str().unwrap());

        let mut f = File::open(path)
            .map_err(|e| Error::chain(format!("Couldn't open template '{:?}'", path), e))?;

        let mut input = String::new();
        f.read_to_string(&mut input)
            .map_err(|e| Error::chain(format!("Failed to read template '{:?}'", path), e))?;

        let tpl = Template::new(tpl_name, Some(path.to_str().unwrap().to_string()), &input)
            .map_err(|e| Error::chain(format!("Failed to parse {:?}", path), e))?;

        self.templates.insert(tpl_name.to_string(), tpl);
        Ok(())
    }

    // Build inheritance chains for loaded templates.
    //
    // We need to know the hierarchy of templates to be able to render multiple extends level.
    // This happens at compile-time to avoid checking it every time we want to render a template.
    // This also checks for soundness issues in the inheritance chains, such as missing template
    // or circular extends.  It also builds the block inheritance chain and detects when super()
    // is called in a place where it can't possibly work
    //
    // You generally don't need to call that yourself, unless you used [`Lysine::parse()`].
    pub fn build_inheritance_chains(&mut self) -> Result<()> {
        // Recursive fn that finds all the parents and put them in an ordered Vec from closest to first parent
        // parent template
        fn build_chain(
            templates: &HashMap<String, Template>,
            start: &Template,
            template: &Template,
            mut parents: Vec<String>,
        ) -> Result<Vec<String>> {
            if !parents.is_empty() && start.name == template.name {
                return Err(Error::circular_extend(&start.name, parents));
            }

            match template.parent {
                Some(ref p) => match templates.get(p) {
                    Some(parent) => {
                        parents.push(parent.name.clone());
                        build_chain(templates, start, parent, parents)
                    }
                    None => Err(Error::missing_parent(&template.name, p)),
                },
                None => Ok(parents),
            }
        }

        // TODO: if we can rewrite the 2 loops below to be only one loop, that'd be great
        let mut tpl_parents = HashMap::new();
        let mut tpl_block_definitions = HashMap::new();
        for (name, template) in &self.templates {
            if template.parent.is_none() && template.blocks.is_empty() {
                continue;
            }

            let parents = build_chain(&self.templates, template, template, vec![])?;

            let mut blocks_definitions = HashMap::new();
            for (block_name, def) in &template.blocks {
                // push our own block first
                let mut definitions = vec![(template.name.clone(), def.clone())];

                // and then see if our parents have it
                for parent in &parents {
                    let t = self.get_template(parent)?;

                    if let Some(b) = t.blocks.get(block_name) {
                        definitions.push((t.name.clone(), b.clone()));
                    }
                }
                blocks_definitions.insert(block_name.clone(), definitions);
            }
            tpl_parents.insert(name.clone(), parents);
            tpl_block_definitions.insert(name.clone(), blocks_definitions);
        }

        for template in self.templates.values_mut() {
            // Simple template: no inheritance or blocks -> nothing to do
            if template.parent.is_none() && template.blocks.is_empty() {
                continue;
            }

            template.parents = tpl_parents.remove(&template.name).unwrap_or_default();
            template.blocks_definitions = tpl_block_definitions.remove(&template.name).unwrap_or_default();
        }

        Ok(())
    }

    // We keep track of macro files loaded in each Template so we can know whether one or them
    // is missing and error accordingly before the user tries to render a template.
    //
    // As with [`build_inheritance_chains()`](Self::build_inheritance_chains), you don't usually need to call that yourself.
    pub fn check_macro_files(&self) -> Result<()> {
        for template in self.templates.values() {
            for (tpl_name, _) in &template.imported_macro_files {
                if !self.templates.contains_key(tpl_name) {
                    return Err(Error::msg(format!(
                        "Template `{}` loads macros from `{}` which isn't present in Lysine",
                        template.name, tpl_name
                    )));
                }
            }
        }

        Ok(())
    }

    pub fn render(&self, template_name: &str, context: &Context) -> Result<String> {
        let template = self.get_template(template_name)?;
        let renderer = Renderer::new(template, self, context);
        renderer.render()
    }

    pub fn render_to(
        &self,
        template_name: &str,
        context: &Context,
        write: impl Write,
    ) -> Result<()> {
        let template = self.get_template(template_name)?;
        let renderer = Renderer::new(template, self, context);
        renderer.render_to(write)
    }

    pub fn render_str(&mut self, input: &str, context: &Context) -> Result<String> {
        self.add_raw_template(ONE_OFF_TEMPLATE_NAME, input)?;
        let result = self.render(ONE_OFF_TEMPLATE_NAME, context);
        self.templates.remove(ONE_OFF_TEMPLATE_NAME);
        result
    }

    pub fn one_off(input: &str, context: &Context, autoescape: bool) -> Result<String> {
        let mut lysine = Lysine::default();

        if autoescape {
            lysine.autoescape_on(vec![ONE_OFF_TEMPLATE_NAME]);
        }

        lysine.render_str(input, context)
    }

    #[doc(hidden)]
    #[inline]
    pub fn get_template(&self, template_name: &str) -> Result<&Template> {
        match self.templates.get(template_name) {
            Some(tpl) => Ok(tpl),
            None => Err(Error::template_not_found(template_name)),
        }
    }

    #[inline]
    pub fn get_template_names(&self) -> impl Iterator<Item = &str> {
        self.templates.keys().map(|s| s.as_str())
    }

    pub fn add_raw_template(&mut self, name: &str, content: &str) -> Result<()> {
        let tpl = Template::new(name, None, content)
            .map_err(|e| Error::chain(format!("Failed to parse '{}'", name), e))?;
        self.templates.insert(name.to_string(), tpl);
        self.build_inheritance_chains()?;
        self.check_macro_files()?;
        Ok(())
    }

    pub fn add_raw_templates<I, N, C>(&mut self, templates: I) -> Result<()>
    where
        I: IntoIterator<Item = (N, C)>,
        N: AsRef<str>,
        C: AsRef<str>,
    {
        for (name, content) in templates {
            let name = name.as_ref();
            let tpl = Template::new(name, None, content.as_ref())
                .map_err(|e| Error::chain(format!("Failed to parse '{}'", name), e))?;
            self.templates.insert(name.to_string(), tpl);
        }
        self.build_inheritance_chains()?;
        self.check_macro_files()?;
        Ok(())
    }

    pub fn add_template_file<P: AsRef<Path>>(&mut self, path: P, name: Option<&str>) -> Result<()> {
        self.add_file(name, path)?;
        self.build_inheritance_chains()?;
        self.check_macro_files()?;
        Ok(())
    }

    pub fn add_template_files<I, P, N>(&mut self, files: I) -> Result<()>
    where
        I: IntoIterator<Item = (P, Option<N>)>,
        P: AsRef<Path>,
        N: AsRef<str>,
    {
        for (path, name) in files {
            self.add_file(name.as_ref().map(AsRef::as_ref), path)?;
        }
        self.build_inheritance_chains()?;
        self.check_macro_files()?;
        Ok(())
    }

    #[doc(hidden)]
    #[inline]
    pub fn get_filter(&self, filter_name: &str) -> Result<&dyn Filter> {
        match self.filters.get(filter_name) {
            Some(fil) => Ok(&**fil),
            None => Err(Error::filter_not_found(filter_name)),
        }
    }

    pub fn register_filter<F: Filter + 'static>(&mut self, name: &str, filter: F) {
        self.filters.insert(name.to_string(), Arc::new(filter));
    }

    #[doc(hidden)]
    #[inline]
    pub fn get_tester(&self, tester_name: &str) -> Result<&dyn Test> {
        match self.testers.get(tester_name) {
            Some(test) => Ok(&**test),
            None => Err(Error::test_not_found(tester_name)),
        }
    }

    pub fn register_tester<T: Test + 'static>(&mut self, name: &str, tester: T) {
        self.testers.insert(name.to_string(), Arc::new(tester));
    }

    #[doc(hidden)]
    #[inline]
    pub fn get_function(&self, fn_name: &str) -> Result<&dyn Function> {
        match self.functions.get(fn_name) {
            Some(fun) => Ok(&**fun),
            None => Err(Error::function_not_found(fn_name)),
        }
    }

    pub fn register_function<F: Function + 'static>(&mut self, name: &str, function: F) {
        self.functions.insert(name.to_string(), Arc::new(function));
    }

    fn register_lysine_filters(&mut self) {
        self.register_filter("upper", string::upper);
        self.register_filter("lower", string::lower);
        self.register_filter("trim", string::trim);
        self.register_filter("trim_start", string::trim_start);
        self.register_filter("trim_end", string::trim_end);
        self.register_filter("trim_start_matches", string::trim_start_matches);
        self.register_filter("trim_end_matches", string::trim_end_matches);
        self.register_filter("truncate", string::truncate);
        self.register_filter("wordcount", string::wordcount);
        self.register_filter("replace", string::replace);
        self.register_filter("capitalize", string::capitalize);
        self.register_filter("title", string::title);
        self.register_filter("linebreaksbr", string::linebreaksbr);
        self.register_filter("indent", string::indent);
        self.register_filter("striptags", string::striptags);
        self.register_filter("spaceless", string::spaceless);
        #[cfg(feature = "urlencode")]
        self.register_filter("urlencode", string::urlencode);
        #[cfg(feature = "urlencode")]
        self.register_filter("urlencode_strict", string::urlencode_strict);
        self.register_filter("escape", string::escape_html);
        self.register_filter("escape_xml", string::escape_xml);
        
        self.register_filter("slugify", string::slugify);
        self.register_filter("addslashes", string::addslashes);
        self.register_filter("split", string::split);
        self.register_filter("int", string::int);
        self.register_filter("float", string::float);

        self.register_filter("first", array::first);
        self.register_filter("last", array::last);
        self.register_filter("nth", array::nth);
        self.register_filter("join", array::join);
        self.register_filter("sort", array::sort);
        self.register_filter("unique", array::unique);
        self.register_filter("slice", array::slice);
        self.register_filter("group_by", array::group_by);
        self.register_filter("filter", array::filter);
        self.register_filter("map", array::map);
        self.register_filter("concat", array::concat);

        self.register_filter("abs", number::abs);
        self.register_filter("pluralize", number::pluralize);
        self.register_filter("round", number::round);

        
        self.register_filter("filesizeformat", number::filesizeformat);

        self.register_filter("length", common::length);
        self.register_filter("reverse", common::reverse);
        
        self.register_filter("date", common::date);
        self.register_filter("json_encode", common::json_encode);
        self.register_filter("as_str", common::as_str);

        self.register_filter("get", object::get);
    }

    fn register_lysine_testers(&mut self) {
        self.register_tester("defined", testers::defined);
        self.register_tester("undefined", testers::undefined);
        self.register_tester("odd", testers::odd);
        self.register_tester("even", testers::even);
        self.register_tester("string", testers::string);
        self.register_tester("number", testers::number);
        self.register_tester("divisibleby", testers::divisible_by);
        self.register_tester("iterable", testers::iterable);
        self.register_tester("object", testers::object);
        self.register_tester("starting_with", testers::starting_with);
        self.register_tester("ending_with", testers::ending_with);
        self.register_tester("containing", testers::containing);
        self.register_tester("matching", testers::matching);
    }

    fn register_lysine_functions(&mut self) {
        self.register_function("range", functions::common::range);
        self.register_function("pick_random", functions::common::pick_random);
        
        self.register_function("now", functions::common::now);
        self.register_function("throw", functions::common::throw);
        
        self.register_function("random_int", functions::common::random_int);
        self.register_function("get_env", functions::common::get_env);
    }

    pub fn autoescape_on(&mut self, suffixes: Vec<&'static str>) {
        self.autoescape_suffixes = suffixes;
    }

    #[doc(hidden)]
    #[inline]
    pub fn get_escape_fn(&self) -> &EscapeFn {
        &self.escape_fn
    }

    pub fn set_escape_fn(&mut self, function: EscapeFn) {
        self.escape_fn = function;
    }

    // Reset escape function to default [`escape_html()`].
    pub fn reset_escape_fn(&mut self) {
        self.escape_fn = escape_html;
    }

    pub fn full_reload(&mut self) -> Result<()> {
        if self.glob.is_some() {
            self.load_from_glob()?;
        } else {
            return Err(Error::msg("Reloading is only available if you are using a glob"));
        }

        self.build_inheritance_chains()?;
        self.check_macro_files()
    }

    pub fn extend(&mut self, other: &Lysine) -> Result<()> {
        for (name, template) in &other.templates {
            if !self.templates.contains_key(name) {
                let mut tpl = template.clone();
                tpl.from_extend = true;
                self.templates.insert(name.to_string(), tpl);
            }
        }

        for (name, filter) in &other.filters {
            if !self.filters.contains_key(name) {
                self.filters.insert(name.to_string(), filter.clone());
            }
        }

        for (name, tester) in &other.testers {
            if !self.testers.contains_key(name) {
                self.testers.insert(name.to_string(), tester.clone());
            }
        }

        for (name, function) in &other.functions {
            if !self.functions.contains_key(name) {
                self.functions.insert(name.to_string(), function.clone());
            }
        }

        self.build_inheritance_chains()?;
        self.check_macro_files()
    }
}

impl Default for Lysine {
    fn default() -> Lysine {
        let mut lysine = Lysine {
            glob: None,
            templates: HashMap::new(),
            filters: HashMap::new(),
            testers: HashMap::new(),
            functions: HashMap::new(),
            autoescape_suffixes: vec![".html", ".htm", ".xml"],
            escape_fn: escape_html,
        };

        lysine.register_lysine_filters();
        lysine.register_lysine_testers();
        lysine.register_lysine_functions();
        lysine
    }
}

// Needs a manual implementation since borrows in Fn's don't implement Debug.
impl fmt::Debug for Lysine {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Lysine {{")?;
        writeln!(f, "\n\ttemplates: [")?;

        for template in self.templates.keys() {
            writeln!(f, "\t\t{},", template)?;
        }
        write!(f, "\t]")?;
        writeln!(f, "\n\tfilters: [")?;

        for filter in self.filters.keys() {
            writeln!(f, "\t\t{},", filter)?;
        }
        write!(f, "\t]")?;
        writeln!(f, "\n\ttesters: [")?;

        for tester in self.testers.keys() {
            writeln!(f, "\t\t{},", tester)?;
        }
        writeln!(f, "\t]")?;

        writeln!(f, "}}")
    }
}

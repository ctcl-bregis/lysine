<img src="lysine.svg" style="width: 25%" alt="Lysine"/>

# Lysine
Lysine is a preprocessor/templating scripting language for webpages.

The Rust version of Lysine itself is a fork of Tera. 

## Name
The name is based on the mechanism of action of the medication, lisdexamfetamine. An analogy would be lisdexamfetamine (lysine markup) is metabolized by red blood cells (compiled) into dextroamphetamine (CSS). The basis of the name is mostly inspired by my interest in ADHD treatments along with it being one of the most well known examples of lysine being used this way.

## Branding
The color to represent Lysine is #FF6600. The secondary color is #0000FF.

## Differences from Tera

Overall code changes:
- Tests have been removed from the source code for now.
- Update code conventions deprecated in Rust 2021 edition 

### Functions
src/builtins/functions.rs was split into the directory src/builtins/functions/ to 

- pick_random: Picks a random value in a vec passed as "array"
- hex2rgb: converts hex (e.g. #F0D000) to RGB values (e.g. rgb(240, 224, 0))

### Syntax

- "True" and "False" are no longer aliases for "true" and "false".